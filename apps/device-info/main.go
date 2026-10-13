package main

import (
	_ "embed"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	gaba "github.com/BrandonKowalski/gabagool/v2/pkg/gabagool"
	"github.com/veandco/go-sdl2/sdl"
)

//go:embed controls.json
var controls []byte

func main() {
	runtime.LockOSThread()
	root := os.Getenv("SLOT_ROOT")
	if root == "" {
		root = "/mnt/sdcard"
	}
	if len(os.Args) > 1 && os.Args[1] == "--text" {
		for _, row := range deviceInfo(root) {
			fmt.Println(row)
		}
		return
	}
	count, err := sdl.GetNumVideoDrivers()
	if err != nil {
		fmt.Fprintln(os.Stderr, "Device Info: SDL video drivers:", err)
		os.Exit(1)
	}
	var drivers []string
	for i := 0; i < count; i++ {
		drivers = append(drivers, sdl.GetVideoDriver(i))
	}
	if len(os.Args) > 1 && os.Args[1] == "--video-drivers" {
		fmt.Println(strings.Join(drivers, "\n"))
		return
	}
	fmt.Fprintf(os.Stderr, "Device Info: initializing SDL (requested=%q, available=%s)\n",
		os.Getenv("SDL_VIDEODRIVER"), strings.Join(drivers, ", "))
	// Gabagool exits silently on SDL_Init failure. Initialize here so the actual
	// backend error is recorded; its later SDL_Init reuses these subsystems.
	if err := sdl.Init(sdl.INIT_VIDEO | sdl.INIT_AUDIO | sdl.INIT_GAMECONTROLLER | sdl.INIT_JOYSTICK); err != nil {
		fmt.Fprintln(os.Stderr, "Device Info: SDL initialization failed:", err)
		os.Exit(1)
	}
	defer sdl.Quit()
	// BaseOS exposes the original H700 kernel button order. Gabagool's NextUI
	// default expects NextUI's remapped order, so supply Slot's physical mapping.
	gaba.SetInputMappingBytes(controls)
	gaba.Init(gaba.Options{
		WindowTitle:          "Device Info",
		IsNextUI:             false,
		PrimaryThemeColorHex: 0x9a9aa4,
		LogPath:              filepath.Join(root, "device-info-ui.log"),
	})
	defer gaba.Close()
	fmt.Fprintln(os.Stderr, "Device Info: UI initialized")
	watchSystemButtons()
	selected := 0
	for {
		var items []gaba.MenuItem
		for _, row := range deviceInfo(root) {
			items = append(items, gaba.MenuItem{Text: row})
		}
		options := gaba.DefaultListOptions("Device Info", items)
		options.SelectedIndex = selected
		options.FooterHelpItems = []gaba.FooterHelpItem{
			{ButtonName: "B", HelpText: "Back"},
			{ButtonName: "A", HelpText: "Refresh"},
		}
		result, err := gaba.List(options)
		if errors.Is(err, gaba.ErrCancelled) {
			fmt.Fprintln(os.Stderr, "Device Info: returning to Slot (Back)")
			return
		}
		if err != nil {
			fmt.Fprintln(os.Stderr, "Device Info:", err)
			return
		}
		// A selects a row to refresh; a window-close event has no selection.
		if result == nil || len(result.Selected) == 0 {
			fmt.Fprintln(os.Stderr, "Device Info: UI closed without a selection")
			return
		}
		selected = result.Selected[0]
	}
}
