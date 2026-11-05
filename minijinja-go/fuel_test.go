package minijinja

import (
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

func fuelEnv(amount uint64) *Environment {
	env := NewEnvironment()
	env.SetFuel(&amount)
	return env
}

func consumedFuel(t *testing.T, env *Environment, source string, ctx any) uint64 {
	t.Helper()
	tmpl, err := env.TemplateFromString(source)
	if err != nil {
		t.Fatalf("parse error: %v", err)
	}
	state, err := tmpl.EvalToState(ctx)
	if err != nil {
		t.Fatalf("render error in %q: %v", source, err)
	}
	consumed, _, ok := state.FuelLevels()
	if !ok {
		t.Fatal("expected fuel tracking to be enabled")
	}
	return consumed
}

func assertOutOfFuel(t *testing.T, err error) {
	t.Helper()
	if kind, ok := rootErrorKind(err); !ok || kind != ErrOutOfFuel {
		t.Fatalf("expected out of fuel error, got %v", err)
	}
}

func TestConsumeFuel(t *testing.T) {
	env := fuelEnv(1000)
	env.AddFunction("burn", func(state *State, args []value.Value, _ map[string]value.Value) (value.Value, error) {
		n, _ := args[0].AsInt()
		if err := state.ConsumeFuel(uint64(n)); err != nil {
			return value.Undefined(), err
		}
		return value.None(), nil
	})

	base := consumedFuel(t, env, "{% do burn(0) %}", nil)
	if got := consumedFuel(t, env, "{% do burn(500) %}", nil); got != base+500 {
		t.Fatalf("expected %d consumed fuel, got %d", base+500, got)
	}

	tmpl, err := env.TemplateFromString("{% do burn(2000) %}")
	if err != nil {
		t.Fatal(err)
	}
	_, err = tmpl.Render(nil)
	assertOutOfFuel(t, err)

	// without fuel tracking this does nothing
	env.SetFuel(nil)
	if _, err := tmpl.Render(nil); err != nil {
		t.Fatal(err)
	}
	var state State
	if err := state.ConsumeFuel(^uint64(0)); err != nil {
		t.Fatal(err)
	}
}

func TestFuelChargesOutput(t *testing.T) {
	env := fuelEnv(100_000)
	big := strings.Repeat("x", 6400) // 100 fuel

	// emitted values
	small := consumedFuel(t, env, "{{ s }}", map[string]any{"s": ""})
	large := consumedFuel(t, env, "{{ s }}", map[string]any{"s": big})
	if large-small != 100 {
		t.Fatalf("expected output to cost 100 fuel, got %d", large-small)
	}

	// template source
	small = consumedFuel(t, env, "-", nil)
	large = consumedFuel(t, env, "-"+big, nil)
	if large-small != 100 {
		t.Fatalf("expected raw output to cost 100 fuel, got %d", large-small)
	}

	// captured output
	small = consumedFuel(t, env, "{% set x %}{{ s }}{% endset %}", map[string]any{"s": ""})
	large = consumedFuel(t, env, "{% set x %}{{ s }}{% endset %}", map[string]any{"s": big})
	if large-small != 100 {
		t.Fatalf("expected captured output to cost 100 fuel, got %d", large-small)
	}

	// the result of a macro is its output which is not charged again
	small = consumedFuel(t, env, "{% macro m() %}{{ s }}{% endmacro %}{% set x = m() %}", map[string]any{"s": ""})
	large = consumedFuel(t, env, "{% macro m() %}{{ s }}{% endmacro %}{% set x = m() %}", map[string]any{"s": big})
	if large-small != 100 {
		t.Fatalf("expected macro output to cost 100 fuel, got %d", large-small)
	}

	// emitting a container is charged as serialization
	items := make([]int, 1000)
	str := value.FromAny(items).String()
	small = consumedFuel(t, env, "{{ x }}", map[string]any{"x": []int{}})
	large = consumedFuel(t, env, "{{ x }}", map[string]any{"x": items})
	want := uint64(len(str)*16-2*16) / 64
	if large-small < want-1 || large-small > want+1 {
		t.Fatalf("expected serialized output to cost about %d fuel, got %d", want, large-small)
	}
}

func TestFuelChargesCreatedData(t *testing.T) {
	env := fuelEnv(100_000)
	ctx := map[string]any{"s": strings.Repeat("x", 6400)}
	base := consumedFuel(t, env, "{% set y = s %}", ctx)
	// the concatenated string costs its size
	if got := consumedFuel(t, env, "{% set y = s ~ s %}", ctx) - base; got < 200 || got > 202 {
		t.Fatalf("expected concatenation to cost about 200 fuel, got %d", got)
	}
	// filters pay for the input string and the result
	if got := consumedFuel(t, env, "{% set y = s|upper %}", ctx) - base; got < 200 || got > 202 {
		t.Fatalf("expected filter to cost about 200 fuel, got %d", got)
	}
	// list literals pay for their slots
	if got := consumedFuel(t, env, "{% set y = [1, 2] %}", ctx) - base; got != 3 {
		t.Fatalf("expected list literal to cost 3 fuel, got %d", got)
	}
}

func TestFuelPreflight(t *testing.T) {
	env := fuelEnv(1000)
	for _, source := range []string{
		"{{ 'x' * 1000000 }}",
		"{{ [1]|batch(1000000, 'x') }}",
		"{{ '%1000000s'|format('x') }}",
		"{{ range(100000) }}",
	} {
		tmpl, err := env.TemplateFromString(source)
		if err != nil {
			t.Fatal(err)
		}
		_, err = tmpl.Render(nil)
		assertOutOfFuel(t, err)
	}
}

func TestFuelStickyExhaustion(t *testing.T) {
	env := fuelEnv(1000)
	// a filter that ignores running out of fuel cannot keep the render going
	env.AddFilter("burn", func(state FilterState, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
		if s, ok := state.(interface{ ConsumeFuel(uint64) error }); ok {
			_ = s.ConsumeFuel(1_000_000)
		}
		return value.FromBool(true), nil
	})
	env.AddTest("burned", func(state FilterState, val value.Value, _ []value.Value) (bool, error) {
		if s, ok := state.(interface{ ConsumeFuel(uint64) error }); ok {
			_ = s.ConsumeFuel(1_000_000)
		}
		return true, nil
	})
	for _, source := range []string{
		"{% set x = 1|burn %}",
		"{% if 1 is burned %}{% endif %}",
		"{{ [1, 2]|select('burned')|list }}",
	} {
		tmpl, err := env.TemplateFromString(source)
		if err != nil {
			t.Fatal(err)
		}
		_, err = tmpl.Render(nil)
		assertOutOfFuel(t, err)
	}
}
