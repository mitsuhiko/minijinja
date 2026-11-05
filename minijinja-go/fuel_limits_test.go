package minijinja

import (
	"errors"
	"iter"
	"runtime"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

// Templates that try to exhaust memory or time must run out of fuel.
//
// This is a port of minijinja/tests/test_fuel_limits.rs.  The allocations
// of a render are measured with runtime.MemStats.TotalAlloc and must stay
// proportional to the fuel it was given.

// bombFuel is the fuel given to the templates in this test.
const bombFuel = 100_000

// bombMaxBytes is the most a bomb may allocate.  The engine charges one fuel
// per 64 bytes.  Operations that cannot check sizes ahead of time may
// overshoot by the size of their result, so allow a generous factor on top.
const bombMaxBytes = bombFuel * 64 * 4

// bombTimeout is how long a bomb may run before it is considered hung.
const bombTimeout = 20 * time.Second

func infiniteIterable() value.Value {
	return value.MakeIterable(func() iter.Seq[value.Value] {
		return func(yield func(value.Value) bool) {
			for i := int64(0); ; i++ {
				if !yield(value.FromInt(i)) {
					return
				}
			}
		}
	})
}

func longIterable() value.Value {
	return value.MakeIterable(func() iter.Seq[value.Value] {
		return func(yield func(value.Value) bool) {
			for i := int64(0); i < 100_000_000; i++ {
				if !yield(value.FromInt(i)) {
					return
				}
			}
		}
	})
}

func bombContext() value.Value {
	bigList := make([]value.Value, 100_000)
	for i := range bigList {
		bigList[i] = value.FromInt(int64(i))
	}
	bigMap := make(map[string]value.Value, 100_000)
	for i := 0; i < 100_000; i++ {
		bigMap[strconv.Itoa(i)] = value.FromInt(int64(i))
	}
	return value.FromMap(map[string]value.Value{
		"infinite":  infiniteIterable(),
		"long_iter": longIterable(),
		"big":       value.FromString(strings.Repeat("x", 1_000_000)),
		"big_list":  value.FromSlice(bigList),
		"big_map":   value.FromMap(bigMap),
	})
}

var fuelBombs = []string{
	// output
	"{% for i in range(100000) %}{{ big }}{% endfor %}",
	"{% for i in range(100000) %}{% set x %}{{ big }}{% endset %}{% endfor %}",
	"{% macro m() %}{{ big }}{% endmacro %}{% for i in range(100000) %}{% set x = m() %}{% endfor %}",
	// repetition and concatenation
	"{{ 'x' * 100000000 }}",
	"{{ ('x' * 100000000)|list }}",
	"{% for i in range(100000) %}{% set s = 'x' * 1000000 %}{% endfor %}",
	"{% set ns = namespace(s='x') %}{% for i in range(60) %}{% set ns.s = ns.s ~ ns.s %}{% endfor %}",
	"{% set ns = namespace(s='x') %}{% for i in range(60) %}{% set ns.s = ns.s + ns.s %}{% endfor %}",
	"{% set ns = namespace(s=[1]) %}{% for i in range(60) %}{% set ns.s = (ns.s + ns.s)|list %}{% endfor %}",
	"{% for i in range(100000) %}{% set x = big ~ big %}{% endfor %}",
	"{{ ((1,) * 100000000)|length }}",
	"{{ ([1] * 100000000)|list|length }}",
	// host iterables
	"{{ infinite|list }}",
	"{{ infinite|sort }}",
	"{{ infinite|join(',') }}",
	"{{ infinite|reverse }}",
	"{{ infinite|last }}",
	"{{ infinite|unique }}",
	"{{ infinite|sum }}",
	"{{ infinite|max }}",
	"{{ infinite|map('string')|list }}",
	"{{ infinite|select('odd')|list }}",
	"{{ infinite|batch(3)|list }}",
	"{{ infinite|slice(3)|list }}",
	"{{ infinite|groupby('x') }}",
	"{{ -1 in infinite }}",
	"{{ infinite[1000000000000] }}",
	"{% for x in infinite %}{% endfor %}",
	"{% for a, b in infinite|batch(2) %}{% endfor %}",
	"{{ long_iter|list|length }}",
	"{{ long_iter|reverse|first }}",
	"{{ long_iter[::-1]|first }}",
	// filters with size arguments
	"{{ [1]|slice(1000000000000) }}",
	"{{ [1]|batch(1000000000000, 'x') }}",
	"{{ 'a\\nb'|indent(1000000000000) }}",
	"{{ '%.1000000000000f'|format(1.0) }}",
	"{{ '%1000000000000d'|format(1) }}",
	"{{ ('a' * 10000)|replace('a', 'b' * 10000) }}",
	"{{ ([1] * 1000000)|join('x' * 1000) }}",
	"{{ ('x' * 100000)|join('y' * 1000) }}",
	// work on big data
	"{% for i in range(100000) %}{{ big_list|sort|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_list|last }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_list|join|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big|upper|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ -1 in big_list }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_list == big_list|list }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_map|dictsort|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_list|unique|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_list|groupby('x')|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_list|map('string')|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_list|select('odd')|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_list[::-1]|first }}{% endfor %}",
	"{% for i in range(100000) %}{{ (big ~ ' ')|split|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_list|tojson|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_list|string|length }}{% endfor %}",

	// Go specific: ranges, reversed lists and iterators are materialized
	"{% for i in range(100000) %}{% set x = range(100000) %}{% endfor %}",
	"{% for i in range(100000) %}{{ big_list|reverse|first }}{% endfor %}",
	"{% for i in range(100000) %}{{ big_map|items|length }}{% endfor %}",
	"{{ infinite|chain([1])|list }}",
	"{{ infinite|zip([1])|list }}",
	"{{ infinite|min }}",
	"{{ infinite + [1] }}",
	"{% set a, b = infinite %}",
	"{{ dict(infinite) }}",
	"{{ infinite is iterable and infinite|first is defined and infinite|join }}",
	"{% for x in oneshot %}{% endfor %}",
	"{% for x in oneshot if x %}{% endfor %}",
	// Go specific: strings are not converted into runes for indexing
	"{% for i in range(100000) %}{{ big[1:2] }}{% endfor %}",
	"{% for i in range(100000) %}{{ big[5] }}{% endfor %}",
	"{% for i in range(100000) %}{{ big[::-1]|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big|list|length }}{% endfor %}",
	"{% for i in range(100000) %}{{ big[-1] }}{% endfor %}",
	"{% for i in range(100000) %}{{ big[10:-10]|length }}{% endfor %}",
	"{% for i in range(100000) %}{% for c in big %}{% break %}{% endfor %}{% endfor %}",
	"{% for i in range(100000) %}{% for k in big_map %}{% break %}{% endfor %}{% endfor %}",
}

func rootErrorKind(err error) (ErrorKind, bool) {
	var kind ErrorKind
	found := false
	for err != nil {
		var mjErr *Error
		if errors.As(err, &mjErr) {
			kind = mjErr.Kind
			found = true
			err = mjErr.Unwrap()
			continue
		}
		break
	}
	return kind, found
}

func renderBomb(t *testing.T, source string) (err error, allocated uint64, elapsed time.Duration) {
	t.Helper()
	env := NewEnvironment()
	fuel := uint64(bombFuel)
	env.SetFuel(&fuel)
	tmpl, err := env.TemplateFromString(source)
	if err != nil {
		t.Fatalf("parse error in %q: %v", source, err)
	}
	ctx := bombContext()
	// the one-shot iterator is not reusable, so it is created per render.
	if m, ok := ctx.AsMap(); ok {
		m["oneshot"] = value.MakeOneShotIterator(func(yield func(value.Value) bool) {
			for i := int64(1); ; i++ {
				if !yield(value.FromInt(i)) {
					return
				}
			}
		})
	}

	type result struct {
		err       error
		allocated uint64
		elapsed   time.Duration
	}
	done := make(chan result, 1)
	go func() {
		runtime.GC()
		var before, after runtime.MemStats
		runtime.ReadMemStats(&before)
		start := time.Now()
		_, err := tmpl.Render(ctx)
		elapsed := time.Since(start)
		runtime.ReadMemStats(&after)
		done <- result{err, after.TotalAlloc - before.TotalAlloc, elapsed}
	}()
	select {
	case rv := <-done:
		return rv.err, rv.allocated, rv.elapsed
	case <-time.After(bombTimeout):
		t.Fatalf("did not finish within %s: %s", bombTimeout, source)
		return nil, 0, 0
	}
}

func TestFuelBombsRunOutOfFuel(t *testing.T) {
	var failures []string
	for _, source := range fuelBombs {
		err, allocated, elapsed := renderBomb(t, source)
		kind, ok := rootErrorKind(err)
		kindName := "ok"
		if ok {
			kindName = kind.String()
		} else if err != nil {
			kindName = err.Error()
		}
		t.Logf("%12d bytes %8.1fms %-14s %s", allocated, float64(elapsed.Microseconds())/1000, kindName, source)
		if !ok || kind != ErrOutOfFuel {
			failures = append(failures, "did not run out of fuel: "+source+" ("+kindName+")")
		}
		if allocated > bombMaxBytes {
			failures = append(failures, "allocated "+strconv.FormatUint(allocated, 10)+" bytes: "+source)
		}
	}
	if len(failures) > 0 {
		t.Fatal(strings.Join(failures, "\n"))
	}
}
