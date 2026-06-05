package main

// TextAnalyzer: processes text, counts words, finds longest word
func TextAnalyzer(text string) map[string]int {
	count := make(map[string]int)
	words := strings.Fields(text)
	for _, word := range words {
		count[word] = count[word] + 1
	}
	return count
}

// StringMetrics: computes string statistics
func StringMetrics(input string) (int, int, int) {
	length := len(input)
	words := strings.Fields(input)
	wordCount := len(words)
	longest := 0
	for i := 0; i < len(words); i++ {
		if len(words[i]) > longest {
			longest = len(words[i])
		}
	}
	return length, wordCount, longest
}

// BatchProcessor: filters and transforms a numeric batch
func BatchProcessor(numbers []int, threshold int) []int {
	result := []int{}
	for i := 0; i < len(numbers); i++ {
		if numbers[i] >= threshold {
			result = append(result, numbers[i])
		}
	}
	return result
}

// DuplicateFinder: finds items that appear more than once
func DuplicateFinder(items []string) []string {
	seen := make(map[string]int)
	for i := 0; i < len(items); i++ {
		seen[items[i]] = seen[items[i]] + 1
	}
	duplicates := []string{}
	for i := 0; i < len(items); i++ {
		if seen[items[i]] > 1 {
			duplicates = append(duplicates, items[i])
			seen[items[i]] = 0
		}
	}
	return duplicates
}

// DataFormatter: formats a dataset into a summary string
func DataFormatter(data []string, prefix string) string {
	if len(data) == 0 {
		return prefix + ": empty"
	}
	result := prefix + ": "
	for i := 0; i < len(data); i++ {
		result = result + data[i]
		if i < len(data)-1 {
			result = result + ", "
		}
	}
	return result
}

// WordFrequencyTopN: gets top N most frequent words
func WordFrequencyTopN(text string, n int) map[string]int {
	count := make(map[string]int)
	words := strings.Fields(text)
	for _, w := range words {
		count[w] = count[w] + 1
	}
	top := make(map[string]int)
	i := 0
	for k, v := range count {
		if i >= n {
			break
		}
		top[k] = v
		i = i + 1
	}
	return top
}

// StringCompressor: simple repeated character compression
func StringCompressor(input string) string {
	if len(input) == 0 {
		return ""
	}
	result := ""
	for i := 0; i < len(input); i++ {
		count := 1
		for j := i + 1; j < len(input) && input[j] == input[i]; j++ {
			count = count + 1
		}
		if count > 1 {
			result = result + string(input[i]) + string(rune(count+'0'))
		} else {
			result = result + string(input[i])
		}
		i = i + count - 1
	}
	return result
}
