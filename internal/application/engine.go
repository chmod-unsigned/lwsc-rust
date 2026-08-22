package application

import (
	"bytes"
	"fmt"
	"image"
	"image/png"
	"log"
	"os/exec"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/go-vgo/robotgo"
	"gocv.io/x/gocv"
	"lwsc-bot/internal/domain"
	"lwsc-bot/internal/infrastructure"
)

type Engine struct {
	config    *infrastructure.Config
	vision    *infrastructure.VisionService
	input     *infrastructure.InputSimulator
	pause     bool
	lastState string
}

func NewEngine(cfg *infrastructure.Config, vs *infrastructure.VisionService, is *infrastructure.InputSimulator) *Engine {
	return &Engine{
		config: cfg,
		vision: vs,
		input:  is,
		pause:  false,
	}
}

func (e *Engine) Run() {
	fmt.Println("Bot engine started...")

	ticker := time.NewTicker(100 * time.Millisecond)
	defer ticker.Stop()

	for range ticker.C {
		if e.pause {
			continue
		}
		
		// Skip processing if the game window is not in focus
		if !IsGameFocused("Last War") {
			continue
		}
		
		var winX, winY, winW, winH int
		
		// 1. Get Game Window
		wx, wy, ww, wh, err := e.GetGameWindowBounds("Last War")
		if err == nil {
			winX, winY, winW, winH = wx, wy, ww, wh
		}
		
		// 2. Grab screen using robotgo (either full screen or window)
		var img image.Image
		if winW > 0 && winH > 0 {
			// Clamp window coordinates to physical screen to prevent X11 BadMatch crash
			screenW, screenH := robotgo.GetScreenSize()
			
			if winX < 0 {
				winW += winX
				winX = 0
			}
			if winY < 0 {
				winH += winY
				winY = 0
			}
			if winX+winW > screenW {
				winW = screenW - winX
			}
			if winY+winH > screenH {
				winH = screenH - winY
			}
			
			if winW > 0 && winH > 0 {
				img, err = robotgo.CaptureImg(winX, winY, winW, winH)
			} else {
				// Window is completely off-screen, fallback to full screen
				img, err = robotgo.CaptureImg()
			}
		} else {
			img, err = robotgo.CaptureImg()
		}
		
		if err != nil {
			continue
		}
		
		// Convert image.Image directly to gocv.Mat (bypassing slow PNG encoding)
		var screenMat gocv.Mat
		if rgba, ok := img.(*image.RGBA); ok {
			mat, err := gocv.NewMatFromBytes(rgba.Bounds().Dy(), rgba.Bounds().Dx(), gocv.MatTypeCV8UC4, rgba.Pix)
			if err == nil {
				screenMat = gocv.NewMat()
				gocv.CvtColor(mat, &screenMat, gocv.ColorRGBAToBGR)
				mat.Close()
			}
		}
		
		if screenMat.Empty() {
			// Fallback if not RGBA or error
			buf := new(bytes.Buffer)
			err = png.Encode(buf, img)
			if err != nil {
				continue
			}
			screenMat, err = gocv.IMDecode(buf.Bytes(), gocv.IMReadColor)
			if err != nil || screenMat.Empty() {
				continue
			}
		}
		
		// 3. Perform template matching concurrently
		var matchedStates []domain.State
		var mu sync.Mutex
		var wg sync.WaitGroup

		for name, state := range e.config.States {
			wg.Add(1)
			go func(stateName string, s domain.State) {
				defer wg.Done()
				matched, err := e.vision.MatchState(screenMat, s)
				if err == nil && matched {
					sCopy := s
					sCopy.Name = stateName
					mu.Lock()
					matchedStates = append(matchedStates, sCopy)
					mu.Unlock()
				}
			}(name, state)
		}

		wg.Wait()

		var detectedState *domain.State
		bestPriority := -1
		
		for _, s := range matchedStates {
			prio := 0
			switch s.Type {
			case "sub_modal":
				prio = 3
			case "modal":
				prio = 2
			case "root":
				prio = 1
			}
			
			if prio > bestPriority {
				bestPriority = prio
				sCopy := s
				detectedState = &sCopy
			}
		}

		if detectedState != nil {
			if detectedState.Name != e.lastState {
				fmt.Printf("\n[%s] [STATE CHANGE] %s -> %s\n", time.Now().Format("15:04:05.000"), e.lastState, detectedState.Name)
				e.lastState = detectedState.Name
			}
			e.executeActionsForState(detectedState, float64(screenMat.Cols()), float64(screenMat.Rows()), winX, winY)
		} else {
			if e.lastState != "UNKNOWN" {
				fmt.Printf("\n[%s] [STATE CHANGE] %s -> UNKNOWN\n", time.Now().Format("15:04:05.000"), e.lastState)
				e.lastState = "UNKNOWN"
			}
		}
		
		screenMat.Close()
	}
}

// Track cooldowns: map[actionName]lastExecutionTime
var actionCooldowns = make(map[string]time.Time)

func (e *Engine) executeActionsForState(state *domain.State, screenW, screenH float64, offsetX, offsetY int) {
	for name, action := range e.config.Actions {
		if !action.Enabled {
			continue
		}

		// Check if action belongs to this state
		isStateMatch := false
		if action.State == state.Name {
			isStateMatch = true
		} else {
			for _, parent := range action.ParentStates {
				if parent == state.Name {
					isStateMatch = true
					break
				}
			}
		}

		if !isStateMatch {
			continue
		}

		// Check cooldown
		if lastExec, exists := actionCooldowns[name]; exists {
			if time.Since(lastExec).Seconds() < action.CooldownS {
				continue // Cooldown active
			}
		}

		fmt.Printf("[%s] Executing action: %s\n", time.Now().Format("15:04:05.000"), name)
		actionCooldowns[name] = time.Now()
		e.ExecuteSingleAction(action, screenW, screenH, offsetX, offsetY)
	}
}

// ExecuteSingleAction executes a specific action (used by both engine loop and keyboard shortcuts)
func (e *Engine) ExecuteSingleAction(action domain.Action, screenW, screenH float64, offsetX, offsetY int) {
	if action.Button != "" {
		// Find button ROI and click
		if btn, ok := e.config.Buttons[action.Button]; ok && btn.ROI != nil {
			x := int((btn.ROI.XMin + btn.ROI.XMax) / 2.0 * screenW)
			y := int((btn.ROI.YMin + btn.ROI.YMax) / 2.0 * screenH)
			e.input.Click(x+offsetX, y+offsetY)
		}
	} else if action.ActionType == "drag_drop" && len(action.DragStart) == 2 && len(action.DragEnd) == 2 {
		startX := int(action.DragStart[0] * screenW)
		startY := int(action.DragStart[1] * screenH)
		endX := int(action.DragEnd[0] * screenW)
		endY := int(action.DragEnd[1] * screenH)
		e.input.Drag(startX+offsetX, startY+offsetY, endX+offsetX, endY+offsetY)
	}
}

// GetGameWindowBounds returns the X, Y, Width, Height of the game window using wmctrl
func (e *Engine) GetGameWindowBounds(title string) (int, int, int, int, error) {
	out, err := exec.Command("wmctrl", "-lG").Output()
	if err != nil {
		return 0, 0, 0, 0, fmt.Errorf("wmctrl failed: %v", err)
	}
	
	lines := strings.Split(string(out), "\n")
	for _, line := range lines {
		if strings.Contains(line, title) {
			parts := strings.Fields(line)
			if len(parts) >= 6 {
				var x, y, w, h int
				fmt.Sscanf(parts[2], "%d", &x)
				fmt.Sscanf(parts[3], "%d", &y)
				fmt.Sscanf(parts[4], "%d", &w)
				fmt.Sscanf(parts[5], "%d", &h)
				return x, y, w, h, nil
			}
		}
	}
	
	return 0, 0, 0, 0, fmt.Errorf("window '%s' not found", title)
}

func (e *Engine) TogglePause() {
	e.pause = !e.pause
	if e.pause {
		log.Printf("[%s] Bot paused.\n", time.Now().Format("15:04:05.000"))
	} else {
		log.Printf("[%s] Bot resumed.\n", time.Now().Format("15:04:05.000"))
	}
}

// IsGameFocused reliably checks if the active window is the game window by comparing X11 Window IDs
func IsGameFocused(title string) bool {
	// 1. Get active window ID
	out, err := exec.Command("xprop", "-root", "_NET_ACTIVE_WINDOW").Output()
	if err != nil {
		return false
	}
	
	parts := strings.Split(string(out), "# ")
	if len(parts) < 2 {
		return false
	}
	
	activeHex := strings.TrimSpace(parts[1])
	activeID, err := strconv.ParseUint(strings.TrimPrefix(activeHex, "0x"), 16, 64)
	if err != nil {
		return false
	}
	
	// 2. Get game window ID from wmctrl
	wmOut, err := exec.Command("wmctrl", "-l").Output()
	if err != nil {
		return false
	}
	
	lines := strings.Split(string(wmOut), "\n")
	for _, line := range lines {
		if strings.Contains(line, title) {
			fields := strings.Fields(line)
			if len(fields) > 0 {
				gameHex := fields[0]
				gameID, err := strconv.ParseUint(strings.TrimPrefix(gameHex, "0x"), 16, 64)
				if err == nil && gameID == activeID {
					return true
				}
			}
		}
	}
	
	return false
}
