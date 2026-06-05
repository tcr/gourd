package main

import (
	"fmt"
	"strings"
)

// HasLongWords: checks if text contains words exceeding a length threshold
func HasLongWords(text string, threshold int) bool {
	words := strings.Fields(text)
	for i := 0; i < len(words); i++ {
		if len(words[i]) > threshold {
			return true
		}
	}
	return false
}

// NormalizeScores: scales a numeric list to [0, 1] range
func NormalizeScores(data []int) []int {
	if len(data) == 0 {
		return []int{}
	}
	hi := data[0]
	lo := data[0]
	for _, v := range data {
		if v > hi {
			hi = v
		}
		if v < lo {
			lo = v
		}
	}
	result := []int{}
	for i := 0; i < len(data); i++ {
		if hi == lo {
			result = append(result, 0)
		} else {
			r := data[i] - lo
			result = append(result, r / (hi - lo))
		}
	}
	return result
}

// FindPeak: finds the index of the highest value
func FindPeak(data []int) int {
	peakIdx := 0
	peakVal := data[0]
	for i := 1; i < len(data); i++ {
		if data[i] > peakVal {
			peakVal = data[i]
			peakIdx = i
		}
	}
	return peakIdx
}

// WordFilter: removes words shorter than minLength
func WordFilter(text string, minLength int) string {
	words := strings.Fields(text)
	result := []string{}
	for _, word := range words {
		if len(word) >= minLength {
			result = append(result, word)
		}
	}
	return strings.Join(result, " ")
}

// RangeClamp: clamps each value within [lo, hi] bounds
func RangeClamp(data []int, lo int, hi int) []int {
	result := []int{}
	for i := 0; i < len(data); i++ {
		v := data[i]
		clamped := min(hi, v)
		clamped = max(lo, clamped)
		result = append(result, clamped)
	}
	return result
}

// WordFreqTopN: returns top N most frequent words by count
func WordFreqTopN(text string, n int) map[string]int {
	counts := make(map[string]int)
	words := strings.Fields(text)
	for _, word := range words {
		counts[word] = counts[word] + 1
	}
	result := make(map[string]int)
	i := 0
	for k, v := range counts {
		if i >= n {
			break
		}
		result[k] = v
		i = i + 1
	}
	return result
}

// SumAll: adds all values in a list
func SumAll(data []int) int {
	sum := 0
	for i := 0; i < len(data); i++ {
		sum = sum + data[i]
	}
	return sum
}

// TrimAndFormat: formats words with a separator
func TrimAndFormat(text string, sep string) string {
	words := strings.Fields(text)
	result := ""
	for i := 0; i < len(words); i++ {
		result = result + words[i]
		if i < len(words)-1 {
			result = result + sep
		}
	}
	return result
}

// Greet: returns a greeting string
func Greet(name string) string {
	return "Hello, " + name + "!"
}

// BuildOutput: assembles status report lines
func BuildOutput(labels []string, values []int, prefix string) string {
	if len(labels) != len(values) {
		return prefix + ": mismatched lengths"
	}
	report := prefix + ":"
	for i := 0; i < len(labels); i++ {
		report = report + labels[i] + "=" + string(rune(values[i]))
		if i < len(labels)-1 {
			report = report + ", "
		}
	}
	return report
}

// DurationReport: converts nanoseconds to human-readable format
func DurationReport(nanos int64) string {
	secs := nanos / 1000000000
	if secs > 0 {
		remaining := int(nanos - secs*1000000000)
		ms := remaining / 1000000
		return fmt.Sprintf("%d.%03ds", secs, ms)
	}
	return fmt.Sprintf("%dns", nanos)
}

func main() {
	text := "the quick brown fox jumps over the lazy dog the fox"
	fmt.Println("Has long words:", HasLongWords(text, 4))
	fmt.Println("Top 3:", WordFreqTopN(text, 3))
	fmt.Println("Trimmed:", TrimAndFormat(text, " | "))
	fmt.Println("Normalized:", NormalizeScores([]int{0, 50, 100}))
	fmt.Println("Peak:", FindPeak([]int{1, 3, 2, 5, 4}))
	fmt.Println("Clamped:", RangeClamp([]int{-5, 0, 5, 10, 15}, 0, 10))
	fmt.Println("Filtered:", WordFilter("hi hello hi there", 3))
	fmt.Println("Summed:", SumAll([]int{1, 2, 3, 4, 5}))
	fmt.Println("Report:", BuildOutput([]string{"a", "b", "c"}, []int{1, 2, 3}, "tag"))
	fmt.Println("Duration:", DurationReport(1500000000))
	fmt.Println("Greet:", Greet("world"))
}
