package value

// UndefinedBehavior determines how undefined values are handled at runtime.
//
// This mirrors the behavior of MiniJinja's Rust implementation and provides
// several modes that control how undefined values interact with printing,
// iteration, attribute access, and truthiness checks.
type UndefinedBehavior int

const (
	// UndefinedLenient allows undefined values to be used in templates.
	// They render as empty strings, iterate as empty sequences, and are
	// considered false in boolean contexts.
	UndefinedLenient UndefinedBehavior = iota

	// UndefinedChainable allows chaining attribute/item access on undefined
	// values without erroring. Missing attributes on undefined remain undefined.
	UndefinedChainable

	// UndefinedSemiStrict behaves like strict undefined values for printing and
	// iteration, but allows undefined values to be used in boolean contexts.
	UndefinedSemiStrict

	// UndefinedStrict causes errors whenever undefined values are used in
	// printing, iteration, or boolean contexts.
	UndefinedStrict
)
