package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestMemory(t *testing.T) {
	for _, test := range []struct{ input, want string }{
		{"MemTotal: 1048576 kB\nMemAvailable: 524288 kB", "512 / 1024 MiB available"},
		{"MemTotal: 1048576 kB", "1024 MiB total"},
		{"MemTotal: 1024 kB\nMemAvailable: 2048 kB", "1 MiB total"},
		{"MemTotal: broken kB", unavailable},
		{"", unavailable},
	} {
		if got := memory(test.input); got != test.want {
			t.Errorf("memory(%q) = %q, want %q", test.input, got, test.want)
		}
	}
}

func TestUptime(t *testing.T) {
	for _, test := range []struct{ input, want string }{
		{"90061.12 123.45", "1d 1h 1m"},
		{"0.1 0.0", "0d 0h 0m"},
		{"-1", unavailable},
		{"broken", unavailable},
		{"NaN", unavailable},
		{"Inf", unavailable},
		{"", unavailable},
	} {
		if got := uptime(test.input); got != test.want {
			t.Errorf("uptime(%q) = %q, want %q", test.input, got, test.want)
		}
	}
}

func TestBatterySkipsOtherPowerSupplies(t *testing.T) {
	root := t.TempDir()
	for name, values := range map[string]map[string]string{
		"ac":      {"type": "Mains", "capacity": "100"},
		"battery": {"type": "Battery", "capacity": "73", "status": "Discharging"},
	} {
		dir := filepath.Join(root, "class/power_supply", name)
		if err := os.MkdirAll(dir, 0755); err != nil {
			t.Fatal(err)
		}
		for key, value := range values {
			if err := os.WriteFile(filepath.Join(dir, key), []byte(value+"\n"), 0644); err != nil {
				t.Fatal(err)
			}
		}
	}
	if got := battery(root); got != "73% (Discharging)" {
		t.Fatal(got)
	}
	if got := battery(t.TempDir()); got != unavailable {
		t.Fatal(got)
	}
}

func TestMissingStorageAndModel(t *testing.T) {
	if got := storage(filepath.Join(t.TempDir(), "missing")); got != unavailable {
		t.Fatal(got)
	}
	file := filepath.Join(t.TempDir(), "model")
	if err := os.WriteFile(file, []byte("Anbernic RG SP\x00"), 0644); err != nil {
		t.Fatal(err)
	}
	if got := readText(file); got != "Anbernic RG SP" {
		t.Fatal(got)
	}
	if got := fields("PRETTY_NAME=\"BaseOS\"\n# comment", "=")["PRETTY_NAME"]; got != "BaseOS" {
		t.Fatal(got)
	}
	if !strings.Contains(storage(t.TempDir()), "GiB free") {
		t.Fatal("existing storage missing")
	}
}
