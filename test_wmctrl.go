package main
import (
	"fmt"
	"os/exec"
	"strings"
	"strconv"
)
func main() {
	out, _ := exec.Command("wmctrl", "-lG").Output()
	fmt.Println(string(out))
}
