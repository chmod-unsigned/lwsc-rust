package infrastructure

import (
	"fmt"
	"strings"

	"github.com/go-vgo/robotgo"
	hook "github.com/robotn/gohook"
)

type InputSimulator struct {
}

func NewInputSimulator() *InputSimulator {
	// Start the global event hook
	hook.Register(hook.KeyDown, []string{}, func(e hook.Event) {
		// General hook listener can be implemented here if needed, but we use specific hooks below.
	})
	
	go func() {
		evChan := hook.Start()
		<-hook.Process(evChan)
	}()
	
	return &InputSimulator{}
}

func (is *InputSimulator) Click(x, y int) {
	robotgo.Move(x, y)
	robotgo.Click("left", false)
}

func (is *InputSimulator) Drag(startX, startY, endX, endY int) {
	robotgo.Move(startX, startY)
	robotgo.DragSmooth(endX, endY)
}

// RegisterShortcut maps string shortcuts (e.g., "ctrl+p") to global hooks.
func (is *InputSimulator) RegisterShortcut(shortcut string, callback func()) error {
	parts := strings.Split(shortcut, "+")
	if len(parts) == 0 {
		return fmt.Errorf("invalid shortcut format")
	}

	key := parts[len(parts)-1]
	var modifiers []string
	if len(parts) > 1 {
		modifiers = parts[:len(parts)-1]
	}

	hook.Register(hook.KeyDown, append(modifiers, key), func(e hook.Event) {
		callback()
	})

	fmt.Printf("Registered global shortcut: %s\n", shortcut)
	return nil
}
