package minijinja

import (
	"context"
	"strings"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/internal/parser"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/syntax"
	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

// OutputSpanKind describes what produced an [OutputSpan].
type OutputSpanKind int

const (
	// OutputSpanRaw is template data that was written verbatim.
	//
	// The output of such a span is identical to the source it points to,
	// so offsets within the span map one-to-one to the template source.
	OutputSpanRaw OutputSpanKind = iota

	// OutputSpanExpression is the output of an expression.
	//
	// This is used for {{ ... }} but also for the output of {% filter %}
	// and {% call %} blocks, in which case the span points to the filter
	// and call expression respectively.
	OutputSpanExpression
)

// String returns the name of the kind.
func (k OutputSpanKind) String() string {
	switch k {
	case OutputSpanRaw:
		return "Raw"
	case OutputSpanExpression:
		return "Expression"
	default:
		return "Unknown"
	}
}

// OutputSpan associates a range of rendered output with the template source
// that produced it.
//
// Output spans are returned by [Template.RenderWithSpans].  Spans are only
// recorded for output that is directly written to the final output.  Output
// that is captured first (such as the output of macros, {% filter %} or
// {% call %} blocks) is attributed to the expression that emitted the
// captured value.  Output of included templates, blocks and super() calls is
// attributed to the template that contains the source that was rendered.
type OutputSpan struct {
	// Name is the name of the template that produced the output.
	Name string
	// Kind describes what kind of source produced the output.
	Kind OutputSpanKind
	// OutputStart is the byte offset in the rendered output where the span
	// starts.
	OutputStart int
	// OutputEnd is the byte offset in the rendered output where the span
	// ends (exclusive).
	OutputEnd int
	// Span is the location in the template source.  For raw spans this is
	// exactly the source that was written, for expression spans this is the
	// source of the expression (without delimiters).
	Span syntax.Span
}

// SourceRange returns the byte range in the template source.
func (s OutputSpan) SourceRange() (start, end int) {
	return int(s.Span.StartOffset), int(s.Span.EndOffset)
}

// Line returns the line (1-indexed) in the template source where the span
// starts.
func (s OutputSpan) Line() int {
	return int(s.Span.StartLine)
}

// spanRecorder collects the final output together with the spans that
// produced it.
type spanRecorder struct {
	buf   strings.Builder
	spans []OutputSpan
}

func (r *spanRecorder) WriteString(s string) (int, error) {
	// see writeString for why the builder is grown this way.
	if r.buf.Cap()-r.buf.Len() < len(s) {
		r.buf.Grow(max(len(s), r.buf.Len()))
	}
	return r.buf.WriteString(s)
}

// writeStringWithSpan writes raw template data and records it.
func (s *State) writeStringWithSpan(str string, span parser.Span) error {
	start, ok := s.spanStart()
	if err := s.writeString(str); err != nil {
		return err
	}
	s.recordSpan(start, ok, OutputSpanRaw, span)
	return nil
}

// writeValueWithSpan writes a value and records it as the output of the
// expression at span.
func (s *State) writeValueWithSpan(val value.Value, span parser.Span) error {
	start, ok := s.spanStart()
	if err := s.writeValue(val); err != nil {
		return err
	}
	s.recordSpan(start, ok, OutputSpanExpression, span)
	return nil
}

// spanStart returns the current position in the final output if spans are
// recorded and the output is not captured.
func (s *State) spanStart() (int, bool) {
	if s.spans == nil || s.out != outputWriter(s.spans) {
		return 0, false
	}
	return s.spans.buf.Len(), true
}

// recordSpan records the output written since start (as returned by
// spanStart) as coming from the given source span.
func (s *State) recordSpan(start int, ok bool, kind OutputSpanKind, span parser.Span) {
	if !ok {
		return
	}
	end := s.spans.buf.Len()
	if start == end {
		return
	}
	s.spans.spans = append(s.spans.spans, OutputSpan{
		Name:        s.name,
		Kind:        kind,
		OutputStart: start,
		OutputEnd:   end,
		Span:        span,
	})
}

// RenderWithSpans renders the template like [Template.Render] but also
// returns the spans of the template source that produced the output.
//
// This is useful when the rendered output is processed further (for instance
// parsed as markup) and problems found there should be reported at the
// template location that produced them.  The returned spans are ordered by
// their position in the output and do not overlap.  Empty output does not
// produce spans.
//
// Output that is captured before it is written (the output of macros,
// {% filter %} and {% call %} blocks) is attributed to the expression that
// writes the captured value.  Output of included templates and blocks is
// attributed to the template that contains the source.
//
// Example:
//
//	tmpl, _ := env.TemplateFromString("Hello {{ name }}!")
//	output, spans, _ := tmpl.RenderWithSpans(map[string]any{"name": "John"})
//	span := spans[1]
//	fmt.Println(output[span.OutputStart:span.OutputEnd]) // John
//	start, end := span.SourceRange()
//	fmt.Println(tmpl.Source()[start:end]) // name
func (t *Template) RenderWithSpans(ctx any) (string, []OutputSpan, error) {
	return t.RenderWithSpansCtx(context.Background(), ctx)
}

// RenderWithSpansCtx is like [Template.RenderWithSpans] but with a
// context.Context.
func (t *Template) RenderWithSpansCtx(goCtx context.Context, ctx any) (string, []OutputSpan, error) {
	state := newState(goCtx, t.env, t.compiled.name, t.compiled.source, value.FromAny(ctx))
	state.spans = &spanRecorder{}
	state.out = state.spans
	output, err := state.eval(t.compiled.ast)
	if err != nil {
		return "", nil, err
	}
	return output, state.spans.spans, nil
}
