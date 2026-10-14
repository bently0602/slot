"""Headless SNES audio comparison; generated homebrew, no game assets."""
import ctypes as C
import array
import json
import pathlib
import shutil
import tempfile
import time
import argparse

ROOT = pathlib.Path(__file__).resolve().parents[1]
CORE = ROOT / 'target/desktop/snes9x_libretro.so'

def rom():
    # SPC700 writes DSP registers, then plays continuous deterministic noise.
    spc = bytearray()
    for reg, value in [(0x6c, 0x3c), (0x0c, 0x60), (0x1c, 0x60),
                       (0x00, 0x60), (0x01, 0x60), (0x05, 0),
                       (0x07, 0x7f), (0x3d, 1), (0x4c, 1)]:
        spc.extend([0x8f, reg, 0xf2, 0x8f, value, 0xf3])
    spc.extend([0x2f, 0xfe])
    code = bytearray([0x78, 0x18, 0xfb, 0xe2, 0x30, 0xa2, 0xff, 0x9a])
    def write(addr, value):
        code.extend([0xa9, value, 0x8d, addr & 255, addr >> 8])
    def wait(value):
        code.extend([0xad, 0x40, 0x21, 0xc9, value, 0xd0, 0xf9])
    write(0x2100, 0x80)
    wait(0xaa)
    write(0x2142, 0)
    write(0x2143, 2)
    write(0x2141, 1)
    write(0x2140, 0xcc)
    wait(0xcc)
    for index, value in enumerate(spc):
        write(0x2141, value)
        write(0x2140, index)
        wait(index)
    write(0x2142, 0)
    write(0x2143, 2)
    write(0x2141, 0)
    write(0x2140, (len(spc) + 1) & 255)
    code.extend([0x80, 0xfe])
    result = bytearray(32768)
    result[:len(code)] = code
    result[0x7fc0:0x7fd5] = b'SLOT SNES AUDIO TEST '
    result[0x7fd5:0x7fdb] = bytes([0x20, 0, 5, 0, 1, 0])
    result[0x7ffc:0x7ffe] = bytes([0, 0x80])
    checksum = (sum(result) + 510) & 65535
    result[0x7fdc:0x7fe0] = (checksum ^ 65535).to_bytes(2, 'little') + checksum.to_bytes(2, 'little')
    return bytes(result)

class Game(C.Structure):
    _fields_ = [('path', C.c_char_p), ('data', C.c_void_p), ('size', C.c_size_t), ('meta', C.c_char_p)]

class Core:
    def __init__(self, path, content, optimized=False):
        self.lib = C.CDLL(str(path))
        self.audio = array.array('h')
        self.callbacks = []
        self.render = True
        self.video_calls = 0
        def env(cmd, data):
            if optimized and cmd == (47 | 0x10000):
                C.cast(data, C.POINTER(C.c_int))[0] = 2 | int(self.render)
                return True
            if optimized and cmd == (72 | 0x10000):
                C.cast(data, C.POINTER(C.c_int))[0] = 2
                return True
            if cmd == 3:  # GET_CAN_DUPE
                C.cast(data, C.POINTER(C.c_bool))[0] = True
                return True
            if cmd == 10:  # SET_PIXEL_FORMAT
                return C.cast(data, C.POINTER(C.c_uint))[0] in (1, 2)
            if cmd == 17:  # GET_VARIABLE_UPDATE
                C.cast(data, C.POINTER(C.c_bool))[0] = False
                return True
            if cmd in (16, 18):
                return True
            return False
        def batch(data, frames):
            self.audio.frombytes(C.string_at(data, frames * 4))
            return frames
        def video(data, width, height, pitch):
            if data:
                self.video_calls += 1
        for name, typ, fn in [
            ('environment', C.CFUNCTYPE(C.c_bool, C.c_uint, C.c_void_p), env),
            ('video_refresh', C.CFUNCTYPE(None, C.c_void_p, C.c_uint, C.c_uint, C.c_size_t), video),
            ('audio_sample', C.CFUNCTYPE(None, C.c_int16, C.c_int16), lambda l, r: self.audio.extend([l, r])),
            ('audio_sample_batch', C.CFUNCTYPE(C.c_size_t, C.POINTER(C.c_int16), C.c_size_t), batch),
            ('input_poll', C.CFUNCTYPE(None), lambda: None),
            ('input_state', C.CFUNCTYPE(C.c_int16, C.c_uint, C.c_uint, C.c_uint, C.c_uint), lambda *args: 0),
        ]:
            cb = typ(fn)
            self.callbacks.append(cb)
            setter = getattr(self.lib, 'retro_set_' + name)
            setter.argtypes = [typ]
            setter(cb)
        self.lib.retro_init()
        self.lib.retro_load_game.argtypes = [C.POINTER(Game)]
        self.lib.retro_load_game.restype = C.c_bool
        self.content = C.create_string_buffer(content)
        game = Game(b'/tmp/slot-snes-tone.sfc', C.cast(self.content, C.c_void_p), len(content), None)
        assert self.lib.retro_load_game(C.byref(game))
        self.lib.retro_serialize_size.restype = C.c_size_t
        self.lib.retro_serialize.argtypes = [C.c_void_p, C.c_size_t]
        self.lib.retro_serialize.restype = C.c_bool
        self.lib.retro_unserialize.argtypes = [C.c_void_p, C.c_size_t]
        self.lib.retro_unserialize.restype = C.c_bool

    def run(self):
        self.lib.retro_run()
        out, self.audio = self.audio, array.array('h')
        return out

    def close(self):
        self.lib.retro_unload_game()
        self.lib.retro_deinit()

def capture(ahead, restore=False, independent=False, save_only=False, optimized=False):
    with tempfile.TemporaryDirectory(prefix='slot-snes-research-') as directory:
        path = pathlib.Path(directory) / 'primary.so'
        shutil.copyfile(CORE, path)
        core = Core(path, rom(), optimized)
        for _ in range(60):
            core.run()
        secondary = None
        if independent:
            other = pathlib.Path(directory) / 'secondary.so'
            shutil.copyfile(CORE, other)
            secondary = Core(other, rom(), optimized)
        state = C.create_string_buffer(core.lib.retro_serialize_size())
        out = array.array('h')
        costs = []
        initial_video_calls = core.video_calls
        for _ in range(180):
            start = time.perf_counter()
            core.render = ahead == 0
            out.extend(core.run())
            if ahead or restore or save_only:
                assert core.lib.retro_serialize(state, len(state))
                dest = secondary or core
                if secondary:
                    assert dest.lib.retro_unserialize(state, len(state))
                for n in range(ahead):
                    dest.render = n + 1 == ahead
                    dest.run()
                if not secondary and not save_only:
                    assert core.lib.retro_unserialize(state, len(state))
            costs.append((time.perf_counter() - start) * 1000)
        if secondary:
            secondary.close()
        primary_video_calls = core.video_calls - initial_video_calls
        core.close()
        return out, sum(costs) / len(costs), primary_video_calls

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--core', type=pathlib.Path, default=CORE)
    args = parser.parse_args()
    CORE = args.core.resolve()
    baseline, cost, videos = capture(0)
    print(json.dumps({'mode': 'baseline', 'samples': len(baseline), 'nonzero': sum(x != 0 for x in baseline), 'distinct_values': len(set(baseline)), 'ms': cost}), flush=True)
    assert any(baseline), 'generated homebrew must produce sound'
    assert len(set(baseline)) > 100, 'audio must vary over time'
    for name, args in [('repeat', (0,)), ('save-only', (0, False, False, True)),
                       ('restore-only', (0, True)), ('single-1', (1,)),
                       ('single-2', (2,)), ('second-1', (1, False, True)),
                       ('second-2', (2, False, True)),
                       ('second-1-video-skip-fast-state', (1, False, True, False, True)),
                       ('second-2-video-skip-fast-state', (2, False, True, False, True))]:
        audio, cost, videos = capture(*args)
        print(json.dumps({'mode': name, 'samples': len(audio), 'changed': sum(a != b for a, b in zip(baseline, audio)), 'equal': baseline == audio, 'primary_video_calls': videos, 'ms': cost}), flush=True)
        assert baseline == audio, name + ' changed the audio stream'
