import pyautogui
import time

IMAGE_PATH = 'assets/retry.png'

i = 1

while True:
    try:
        position = pyautogui.locateCenterOnScreen(IMAGE_PATH, confidence=0.8)
        
        if position is not None:
            print(f"{i}1. Retry button detected at {position}\n")
            pyautogui.click(position)
            pyautogui.moveTo(10, 10)
            time.sleep(2)
            i = i + 1
            
    except (pyautogui.ImageNotFoundException, TypeError):
        pass
    except KeyboardInterrupt:
        print("Shutting down...\n")
        break
    except Exception as e:
        print(f"Unknown error: {e}\n")
    
    time.sleep(1)
