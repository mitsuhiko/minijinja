package errors

import (
	goerrors "errors"
	"fmt"
	"sort"
	"strings"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2/syntax"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
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

	if info.TemplateSource != "" {
		title := fmt.Sprintf(" %s ", templateTitle(err.Name))
		_, _ = fmt.Fprint(f, "\n")
		_, _ = fmt.Fprintln(f, centerLine(title, '-', 79))

		lines := strings.Split(info.TemplateSource, "\n")
		lineIdx := 0
		if err.Span != nil && err.Span.StartLine > 0 {
			lineIdx = int(err.Span.StartLine - 1)
		}
		if lineIdx >= len(lines) {
			lineIdx = len(lines) - 1
		}
		if lineIdx < 0 {
			lineIdx = 0
		}

		skip := lineIdx - 3
		if skip < 0 {
			skip = 0
		}
		for idx := skip; idx < lineIdx && idx < len(lines); idx++ {
			_, _ = fmt.Fprintf(f, "%4d | %s\n", idx+1, lines[idx])
		}

		if lineIdx < len(lines) {
			_, _ = fmt.Fprintf(f, "%4d > %s\n", lineIdx+1, lines[lineIdx])
		}

		if err.Span != nil && err.Span.StartLine == err.Span.EndLine {
			_, _ = fmt.Fprintf(
				f,
				"     i %s%s %s\n",
				strings.Repeat(" ", int(err.Span.StartCol)),
				strings.Repeat("^", caretWidth(err.Span)),
				err.Kind,
			)
		}

		for idx := lineIdx + 1; idx <= lineIdx+3 && idx < len(lines); idx++ {
			_, _ = fmt.Fprintf(f, "%4d | %s\n", idx+1, lines[idx])
		}
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
		_, _ = fmt.Fprintf(f, "    %s: %s\n", key, locals[key].Repr())
	}
}

func caretWidth(span *syntax.Span) int {
	if span == nil {
		return 0
	}
	if span.EndCol <= span.StartCol {
		return 1
	}
	return int(span.EndCol - span.StartCol)
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
