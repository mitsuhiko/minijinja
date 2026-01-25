// Example: Using context.Context with templates
//
// This example demonstrates how to use context.Context for:
// - Passing request-scoped values to custom filters/functions
// - Implementing cancellation and timeouts
package main

import (
	"context"
	"fmt"
	"log"
	"time"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

// requestIDKey is a custom context key for request IDs.
type requestIDKey struct{}

func main() {
	env := minijinja.NewEnvironment()

	// Add a custom function that accesses the context.Context.
	// This is useful for accessing request-scoped data like request IDs,
	// user information, database connections, etc.
	env.AddFunction("request_id", func(state *minijinja.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		ctx := state.Context()
		if id, ok := ctx.Value(requestIDKey{}).(string); ok {
			return value.FromString(id), nil
		}
		return value.FromString("unknown"), nil
	})

	// Add a custom filter that respects context cancellation.
	// This is important for long-running operations.
	env.AddFilter("slow_process", func(state minijinja.FilterState, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		ctx := state.Context()

		// Simulate a slow operation that checks for cancellation
		select {
		case <-ctx.Done():
			return value.Undefined(), ctx.Err()
		case <-time.After(10 * time.Millisecond):
			// Process completed
			s, _ := val.AsString()
			return value.FromString("[processed: " + s + "]"), nil
		}
	})

	err := env.AddTemplate("example.txt", `Request ID: {{ request_id() }}
Data: {{ data|slow_process }}
`)
	if err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("example.txt")
	if err != nil {
		log.Fatal(err)
	}

	// Create a context with a request ID
	ctx := context.WithValue(context.Background(), requestIDKey{}, "req-12345")

	// Render using the context-aware method
	result, err := tmpl.RenderCtx(ctx, map[string]any{
		"data": "hello world",
	})
	if err != nil {
		log.Fatalf("Render failed: %v", err)
	}
	fmt.Println("=== Normal rendering ===")
	fmt.Println(result)

	// Example with timeout - this would fail if slow_process took too long
	ctxWithTimeout, cancel := context.WithTimeout(ctx, 100*time.Millisecond)
	defer cancel()

	result2, err := tmpl.RenderCtx(ctxWithTimeout, map[string]any{
		"data": "with timeout",
	})
	if err != nil {
		log.Fatalf("Render with timeout failed: %v", err)
	}
	fmt.Println("=== Rendering with timeout ===")
	fmt.Println(result2)

	// Example showing cancellation (commented out as it would fail)
	// ctxCancelled, cancelNow := context.WithCancel(ctx)
	// cancelNow() // Cancel immediately
	// _, err = tmpl.RenderCtx(ctxCancelled, map[string]any{"data": "test"})
	// fmt.Println("Cancelled error:", err) // Would print: context canceled
}
