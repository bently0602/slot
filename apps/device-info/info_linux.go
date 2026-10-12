package main

import (
	"fmt"
	"math"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"syscall"
	"time"
)

const unavailable = "Unavailable"

func readText(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	return strings.Trim(strings.TrimSpace(string(data)), "\x00")
}

func fields(text, separator string) map[string]string {
	result := make(map[string]string)
	for _, line := range strings.Split(text, "\n") {
		key, value, found := strings.Cut(line, separator)
		if found {
			result[strings.TrimSpace(key)] = strings.Trim(strings.TrimSpace(value), "\"'")
		}
	}
	return result
}

func first(values ...string) string {
	for _, value := range values {
		if value != "" {
			return value
		}
	}
	return unavailable
}

func memory(text string) string {
	values := fields(text, ":")
	number := func(key string) (uint64, bool) {
		parts := strings.Fields(values[key])
		if len(parts) != 2 || parts[1] != "kB" {
			return 0, false
		}
		value, err := strconv.ParseUint(parts[0], 10, 64)
		return value, err == nil
	}
	total, ok := number("MemTotal")
	if !ok || total == 0 {
		return unavailable
	}
	available, ok := number("MemAvailable")
	if !ok || available > total {
		return fmt.Sprintf("%d MiB total", total/1024)
	}
	return fmt.Sprintf("%d / %d MiB available", available/1024, total/1024)
}

func uptime(text string) string {
	parts := strings.Fields(text)
	if len(parts) == 0 {
		return unavailable
	}
	seconds, err := strconv.ParseFloat(parts[0], 64)
	if err != nil || math.IsNaN(seconds) || math.IsInf(seconds, 0) || seconds < 0 || seconds > 1e12 {
		return unavailable
	}
	minutes := uint64(seconds / 60)
	return fmt.Sprintf("%dd %dh %dm", minutes/1440, minutes/60%24, minutes%60)
}

func battery(sysRoot string) string {
	supplies, _ := filepath.Glob(filepath.Join(sysRoot, "class/power_supply/*"))
	for _, supply := range supplies {
		if readText(filepath.Join(supply, "type")) != "Battery" {
			continue
		}
		capacity, err := strconv.Atoi(readText(filepath.Join(supply, "capacity")))
		if err != nil || capacity < 0 || capacity > 100 {
			continue
		}
		status := readText(filepath.Join(supply, "status"))
		if status == "" {
			return fmt.Sprintf("%d%%", capacity)
		}
		return fmt.Sprintf("%d%% (%s)", capacity, status)
	}
	return unavailable
}

func storage(root string) string {
	var stat syscall.Statfs_t
	if err := syscall.Statfs(root, &stat); err != nil || stat.Blocks == 0 || stat.Bsize <= 0 {
		return unavailable
	}
	gib := float64(stat.Bsize) / (1024 * 1024 * 1024)
	return fmt.Sprintf("%.1f / %.1f GiB free", float64(stat.Bavail)*gib, float64(stat.Blocks)*gib)
}

func deviceInfo(root string) []string {
	cpu := fields(readText("/proc/cpuinfo"), ":")
	release := fields(readText("/etc/os-release"), "=")
	baseOS := fields(readText("/etc/baseos-release"), "=")
	return []string{
		"Device: " + first(baseOS["BASEOS_MODEL"], readText("/etc/model"), readText("/proc/device-tree/model")),
		"Slot: " + first(os.Getenv("SLOT_VERSION")),
		"Build: " + first(os.Getenv("SLOT_BUILD")),
		"OS: " + first(release["PRETTY_NAME"], release["NAME"]),
		"Kernel: " + first(readText("/proc/sys/kernel/osrelease")),
		"CPU: " + first(cpu["model name"], cpu["Hardware"], cpu["Processor"]),
		fmt.Sprintf("Architecture: %s (%d CPUs)", runtime.GOARCH, runtime.NumCPU()),
		"Memory: " + memory(readText("/proc/meminfo")),
		"Uptime: " + uptime(readText("/proc/uptime")),
		"Battery: " + battery("/sys"),
		"Framebuffer: " + first(readText("/sys/class/graphics/fb0/virtual_size")),
		"SD card: " + storage(root),
		"Content root: " + root,
		"Updated: " + time.Now().Format("15:04:05"),
	}
}
