package main

import (
	_ "embed"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"

	gaba "github.com/BrandonKowalski/gabagool/v2/pkg/gabagool"
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
			return
		}
		if err != nil {
			fmt.Fprintln(os.Stderr, "Device Info:", err)
			return
		}
		// A selects a row to refresh; a window-close event has no selection.
		if result == nil || len(result.Selected) == 0 {
			return
		}
		selected = result.Selected[0]
	}
}
