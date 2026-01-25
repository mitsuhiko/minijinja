package syntax

// Span represents a location range in source code.
type Span struct {
	StartLine   uint16
	StartCol    uint16
	StartOffset uint32
	EndLine     uint16
	EndCol      uint16
	EndOffset   uint32
}
