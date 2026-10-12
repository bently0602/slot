package main

import (
	"os"
	"path/filepath"
	"time"

	"github.com/holoplot/go-evdev"
)

// Return to Slot for power and lid handling instead of using NextUI's suspend
// script. These readers never grab the devices or change their configuration.
func watchSystemButtons() {
	if os.Getenv("PLATFORM") != "h700" {
		return
	}
	nodes, _ := filepath.Glob("/dev/input/event*")
	for _, node := range nodes {
		device, err := evdev.OpenWithFlags(node, os.O_RDONLY)
		if err != nil {
			continue
		}
		go func() {
			defer device.Close()
			for {
				event, err := device.ReadOne()
				if err != nil {
					return
				}
				if event.Type == evdev.EV_KEY && event.Code == evdev.KEY_POWER && event.Value == 0 {
					// The launcher asks Slot to sleep after recreating its
					// platform. Process exit also releases every SDL resource.
					os.Exit(64)
				}
				if event.Type == evdev.EV_SW && event.Code == evdev.SW_LID && event.Value == 1 {
					os.Exit(0)
				}
			}
		}()
	}
	go func() {
		halls, _ := filepath.Glob("/sys/class/power_supply/*/hallkey")
		ticker := time.NewTicker(150 * time.Millisecond)
		defer ticker.Stop()
		for range ticker.C {
			for _, hall := range halls {
				if readText(hall) == "0" {
					os.Exit(0)
				}
			}
		}
	}()
}
