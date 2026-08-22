package infrastructure

import (
	"fmt"
	"image"
	"os"
	"path/filepath"
	"strings"

	"gocv.io/x/gocv"
	"lwsc-bot/internal/domain"
)

type VisionService struct {
	templates map[string][]gocv.Mat
}

func NewVisionService(configDir string, cfg *Config) (*VisionService, error) {
	vs := &VisionService{
		templates: make(map[string][]gocv.Mat),
	}
	
	baseDir := filepath.Dir(configDir)

	// Pre-load templates for states
	for name, state := range cfg.States {
		if state.Templates != "" {
			path := filepath.Join(baseDir, state.Templates)
			
			fileInfo, err := os.Stat(path)
			if err != nil {
				continue
			}

			if fileInfo.IsDir() {
				// Load all pngs in directory
				files, _ := filepath.Glob(filepath.Join(path, "*.png"))
				for _, f := range files {
					mat := gocv.IMRead(f, gocv.IMReadColor)
					if !mat.Empty() {
						vs.templates[name] = append(vs.templates[name], mat)
					}
				}
			} else if strings.HasSuffix(path, ".png") {
				mat := gocv.IMRead(path, gocv.IMReadColor)
				if !mat.Empty() {
					vs.templates[name] = append(vs.templates[name], mat)
				}
			}
		}
	}

	return vs, nil
}

func (vs *VisionService) MatchState(screen gocv.Mat, state domain.State) (bool, error) {
	templates, ok := vs.templates[state.Name]
	if !ok || len(templates) == 0 {
		return false, fmt.Errorf("no templates found for state %s", state.Name)
	}
	
	if screen.Empty() {
		return false, fmt.Errorf("screen image is empty")
	}

	// Extreme optimization: Crop screen to ROI if defined
	targetScreen := screen
	if state.ROI != nil {
		cols := float64(screen.Cols())
		rows := float64(screen.Rows())
		
		rect := image.Rect(
			int(state.ROI.XMin * cols),
			int(state.ROI.YMin * rows),
			int(state.ROI.XMax * cols),
			int(state.ROI.YMax * rows),
		)
		
		// Ensure rect is within bounds
		if rect.Min.X >= 0 && rect.Min.Y >= 0 && rect.Max.X <= screen.Cols() && rect.Max.Y <= screen.Rows() {
			targetScreen = screen.Region(rect)
			defer targetScreen.Close()
		}
	}

	var bestConf float64
	for _, template := range templates {
		if template.Rows() > targetScreen.Rows() || template.Cols() > targetScreen.Cols() {
			continue // Template larger than search area
		}

		result := gocv.NewMat()
		gocv.MatchTemplate(targetScreen, template, &result, gocv.TmCcoeffNormed, gocv.NewMat())
		_, maxVal, _, _ := gocv.MinMaxLoc(result)
		result.Close()

		confidence := float64(maxVal)
		if confidence > bestConf {
			bestConf = confidence
		}

		threshold := state.MinConfidence
		if threshold == 0 {
			threshold = 0.8
		}

		if confidence >= threshold {
			// fmt.Printf("[DEBUG] State %s matched with conf %.2f (thresh %.2f)\n", state.Name, confidence, threshold)
			return true, nil
		}
	}
	
	return false, nil
}
