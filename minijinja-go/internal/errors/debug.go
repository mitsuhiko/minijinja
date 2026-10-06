package errors

import (
	goerrors "errors"
	"fmt"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/syntax"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

// DebugInfo is a snapshot of debug information captured during rendering.
type DebugInfo struct {
	TemplateSource   string
	ReferencedLocals map[string]value.Value
}

func formatErrorWithDebug(f fmt.State, err *Error, includeChain bool) {
	_, _ = fmt.Fprint(f, err.Error())
	if err.DebugInfo != nil {
		renderDebugInfo(f, err)
	}

	if includeChain {
		for cause := goerrors.Unwrap(err); cause != nil; cause = goerrors.Unwrap(cause) {
			_, _ = fmt.Fprint(f, "\n\ncaused by: ")
			if next, ok := cause.(*Error); ok {
				formatErrorWithDebug(f, next, false)
			} else {
				_, _ = fmt.Fprintf(f, "%v", cause)
			}
		}
	}
}

func renderDebugInfo(f fmt.State, err *Error) {
	info := err.DebugInfo
	if info == nil {
		return
	}

	// without a location there is nothing in the source to point to
	if info.TemplateSource != "" && err.Span != nil && err.Span.StartLine > 0 {
		title := fmt.Sprintf(" %s ", templateTitle(err.Name))
		_, _ = fmt.Fprint(f, "\n")
		_, _ = fmt.Fprintln(f, centerLine(title, '-', 79))
		renderSourceExcerpt(f, info.TemplateSource, err.Span, err.Kind)
		_, _ = fmt.Fprint(f, strings.Repeat("~", 79))
	}

	_, _ = fmt.Fprint(f, "\n")
	renderReferencedLocals(f, info.ReferencedLocals)
	_, _ = fmt.Fprint(f, strings.Repeat("-", 79))
}

func renderReferencedLocals(f fmt.State, locals map[string]value.Value) {
	if len(locals) == 0 {
		_, _ = fmt.Fprint(f, "No referenced variables\n")
		return
	}

	_, _ = fmt.Fprint(f, "Referenced variables:\n")
	keys := make([]string, 0, len(locals))
	for key := range locals {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	for _, key := range keys {
		_, _ = fmt.Fprintf(f, "    %s: %s\n", key, limitedRepr(locals[key], 0))
	}
}

// renderSourceExcerpt renders the lines around the start of the span.
func renderSourceExcerpt(f fmt.State, source string, span *syntax.Span, kind ErrorKind) {
	const contextLines = 3

	lines := strings.Split(source, "\n")
	lineIdx := min(int(span.StartLine-1), len(lines)-1)
	for idx := max(lineIdx-contextLines, 0); idx < lineIdx; idx++ {
		_, _ = fmt.Fprintf(f, "%4d | %s\n", idx+1, lines[idx])
	}
	_, _ = fmt.Fprintf(f, "%4d > %s\n", lineIdx+1, lines[lineIdx])
	if lineIdx == int(span.StartLine-1) {
		renderSpanMarker(f, lines[lineIdx], span, kind)
	}
	for idx := lineIdx + 1; idx <= lineIdx+contextLines && idx < len(lines); idx++ {
		_, _ = fmt.Fprintf(f, "%4d | %s\n", idx+1, lines[idx])
	}
}

// renderSpanMarker underlines the span on its first line.
//
// Spans that continue on later lines are underlined to the end of the line.
// Tabs before the span are retained so that the marker lines up with the
// source line.
func renderSpanMarker(f fmt.State, line string, span *syntax.Span, kind ErrorKind) {
	start := int(span.StartCol)
	end := int(span.EndCol)
	if span.EndLine != span.StartLine {
		end = utf8.RuneCountInString(line)
	}

	var prefix strings.Builder
	for _, c := range line {
		if prefix.Len() >= start {
			break
		}
		if c == '\t' {
			prefix.WriteByte('\t')
		} else {
			prefix.WriteByte(' ')
		}
	}
	for prefix.Len() < start {
		prefix.WriteByte(' ')
	}

	_, _ = fmt.Fprintf(
		f,
		"     i %s%s %s\n",
		prefix.String(),
		strings.Repeat("^", max(end-start, 1)),
		kind,
	)
}

func templateTitle(name string) string {
	if name == "" {
		return "Template Source"
	}
	parts := strings.FieldsFunc(name, func(r rune) bool { return r == '/' || r == '\\' })
	if len(parts) == 0 {
		return "Template Source"
	}
	return parts[len(parts)-1]
}

func centerLine(title string, fill rune, width int) string {
	if len(title) >= width {
		return title
	}
	pad := width - len(title)
	left := pad / 2
	right := pad - left
	return strings.Repeat(string(fill), left) + title + strings.Repeat(string(fill), right)
}

const (
	// debugMaxItems is the maximum number of items shown per sequence or map.
	debugMaxItems = 10
	// debugMaxDepth is the maximum nesting depth shown.
	debugMaxDepth = 4
	// debugMaxStringChars is the maximum number of characters shown per string.
	debugMaxStringChars = 100
)

// limitedRepr renders a value like Repr but limits its size so that large
// values (eg: the entire environment) do not drown out the relevant
// information in debug output.
func limitedRepr(v value.Value, depth int) string {
	switch v.Kind() {
	case value.KindString:
		s, _ := v.AsString()
		if utf8.RuneCountInString(s) <= debugMaxStringChars {
			return v.Repr()
		}
		cut := 0
		for i := 0; i < debugMaxStringChars; i++ {
			_, size := utf8.DecodeRuneInString(s[cut:])
			cut += size
		}
		return value.FromString(s[:cut]).Repr() + "..."
	case value.KindSeq:
		items, ok := v.AsSlice()
		if !ok {
			return v.Repr()
		}
		open, close := "[", "]"
		if v.IsTuple() {
			open, close = "(", ")"
			if len(items) == 1 && depth < debugMaxDepth {
				close = ",)"
			}
		}
		if depth >= debugMaxDepth {
			return open + "..." + close
		}
		parts := make([]string, 0, debugMaxItems+1)
		for i, item := range items {
			if i == debugMaxItems {
				parts = append(parts, fmt.Sprintf("... %d more", len(items)-i))
				break
			}
			parts = append(parts, limitedRepr(item, depth+1))
		}
		return open + strings.Join(parts, ", ") + close
	case value.KindMap:
		m, ok := v.AsMap()
		if !ok {
			return v.Repr()
		}
		if depth >= debugMaxDepth {
			return "{...}"
		}
		keys := make([]string, 0, len(m))
		for key := range m {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		parts := make([]string, 0, debugMaxItems+1)
		for i, key := range keys {
			if i == debugMaxItems {
				parts = append(parts, fmt.Sprintf("...: %d more", len(keys)-i))
				break
			}
			parts = append(parts, fmt.Sprintf("%s: %s", debugMapKey(key), limitedRepr(m[key], depth+1)))
		}
		return "{" + strings.Join(parts, ", ") + "}"
	default:
		return v.Repr()
	}
}

// debugMapKey formats a map key like Repr does.
func debugMapKey(key string) string {
	if i, err := strconv.ParseInt(key, 10, 64); err == nil && strconv.FormatInt(i, 10) == key {
		return key
	}
	return value.FromString(key).Repr()
}
