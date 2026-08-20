#!/usr/bin/env python3
import sys
import os
import time
import argparse
import subprocess
import cv2
import numpy as np

def get_window_geometry(window_id):
    """Retrieve absolute X, Y coordinates and dimensions of the X11 target window."""
    if not window_id:
        return 0, 0, 0, 0
    try:
        out = subprocess.check_output(["xdotool", "getwindowgeometry", str(window_id)], text=True)
        # Parse output format:
        # Window 94371843
        #   Position: 200,150 (screen: 0)
        #   Geometry: 540x960
        pos_line = [l for l in out.splitlines() if "Position:" in l]
        geo_line = [l for l in out.splitlines() if "Geometry:" in l]
        win_x, win_y = 0, 0
        win_w, win_h = 0, 0
        if pos_line:
            pos_part = pos_line[0].split("Position:")[1].split()[0]
            win_x, win_y = map(int, pos_part.split(","))
        if geo_line:
            geo_part = geo_line[0].split("Geometry:")[1].split()[0]
            win_w, win_h = map(int, geo_part.split("x"))
        return win_x, win_y, win_w, win_h
    except Exception as e:
        print(f"[Python Custom Action] Warning: Could not query window geometry for {window_id}: {e}")
        return 0, 0, 0, 0

def click_at(x, y):
    """Perform a clean mouse click at absolute screen coordinates."""
    try:
        subprocess.run(["xdotool", "mousemove", str(x), str(y), "click", "1"], check=True)
    except Exception:
        try:
            import pyautogui
            pyautogui.click(x, y)
        except Exception as e:
            print(f"[Python Custom Action] Failed to click at ({x}, {y}): {e}")

def find_all_template_occurrences(screenshot_img, template_img, threshold=0.75, min_dist=30):
    """Find all unique non-overlapping template matches with Non-Maximum Suppression."""
    img_gray = cv2.cvtColor(screenshot_img, cv2.COLOR_BGR2GRAY) if len(screenshot_img.shape) == 3 else screenshot_img
    tmpl_gray = cv2.cvtColor(template_img, cv2.COLOR_BGR2GRAY) if len(template_img.shape) == 3 else template_img
    
    th, tw = tmpl_gray.shape[:2]
    res = cv2.matchTemplate(img_gray, tmpl_gray, cv2.TM_CCOEFF_NORMED)
    
    loc = np.where(res >= threshold)
    points = []
    
    # Collect all candidate match centers and confidences
    candidates = []
    for pt in zip(*loc[::-1]):
        score = float(res[pt[1], pt[0]])
        center_x = int(pt[0] + tw / 2)
        center_y = int(pt[1] + th / 2)
        candidates.append((score, center_x, center_y))
        
    # Sort candidates by descending confidence
    candidates.sort(key=lambda c: c[0], reverse=True)
    
    # Non-maximum suppression by distance
    for score, cx, cy in candidates:
        too_close = False
        for _, ecx, ecy in points:
            dist = np.hypot(cx - ecx, cy - ecy)
            if dist < min_dist:
                too_close = True
                break
        if not too_close:
            points.append((score, cx, cy))
            
    return points, (tw, th)

def main():
    parser = argparse.ArgumentParser(description="LWSC2 Custom Action: Search Gold Mine")
    parser.add_argument("--window-id", type=str, help="Target game window ID")
    parser.add_argument("--state", type=str, help="Current GameState name")
    parser.add_argument("--screenshot", type=str, default="last_screenshot.png", help="Path to latest screenshot PNG")
    parser.add_argument("--template", type=str, default="poi/gold_mine.png", help="Path to gold mine template")
    parser.add_argument("--threshold", type=float, default=0.70, help="Matching confidence threshold (0.0 - 1.0)")
    parser.add_argument("--delay", type=float, default=1.0, help="Delay in seconds between clicks")
    args, unknown = parser.parse_known_args()

    print(f"[Python Custom Action] === Search Gold Mine Started ===")
    print(f"[Python Custom Action] Window: {args.window_id}, State: {args.state}")

    # 1. Verify screenshot path
    screenshot_path = args.screenshot
    if not os.path.exists(screenshot_path):
        if os.path.exists("last_screenshot.png"):
            screenshot_path = "last_screenshot.png"
        else:
            print(f"[Python Custom Action] Error: Screenshot '{args.screenshot}' not found.")
            return

    # 2. Verify template path
    template_path = args.template
    if not os.path.exists(template_path):
        if os.path.exists("gold_mine.png"):
            template_path = "gold_mine.png"
        elif os.path.exists("poi/gold_mine.png"):
            template_path = "poi/gold_mine.png"
        else:
            print(f"[Python Custom Action] Error: Template '{args.template}' not found.")
            return

    screenshot = cv2.imread(screenshot_path)
    template = cv2.imread(template_path)

    if screenshot is None:
        print(f"[Python Custom Action] Error: Could not read screenshot image '{screenshot_path}'.")
        return
    if template is None:
        print(f"[Python Custom Action] Error: Could not read template image '{template_path}'.")
        return

    # 3. Find all occurrences
    th, tw = template.shape[:2]
    min_dist = max(tw, th, 30)
    matches, (tw, th) = find_all_template_occurrences(screenshot, template, threshold=args.threshold, min_dist=min_dist)
    print(f"[Python Custom Action] Found {len(matches)} gold mine occurrence(s) with confidence >= {args.threshold * 100:.1f}%.")

    if not matches:
        print("[Python Custom Action] No gold mine detected in current frame.")
        return

    # 4. Get target window offset
    win_x, win_y, win_w, win_h = get_window_geometry(args.window_id)
    print(f"[Python Custom Action] Target Window Origin: ({win_x}, {win_y})")

    # 5. Iterate over each detected gold mine, click, wait 1s, and reiterate
    for idx, (score, rx, ry) in enumerate(matches, 1):
        abs_x = win_x + rx
        abs_y = win_y + ry
        print(f"\n[Python Custom Action] [{idx}/{len(matches)}] Clicking Gold Mine (conf: {score * 100:.1f}%) at window ({rx}, {ry}) -> screen ({abs_x}, {abs_y})")
        
        click_at(abs_x, abs_y)
        
        if idx < len(matches):
            print(f"[Python Custom Action] Waiting {args.delay}s before next click...")
            time.sleep(args.delay)

    print("\n[Python Custom Action] === Search Gold Mine Completed Successfully ===")

if __name__ == "__main__":
    main()


