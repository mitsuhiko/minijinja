package minijinja

import (
	"fmt"
	"iter"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

// Measures how fuel relates to time and memory.
//
// This is a port of benchmarks/src/bin/fuel_calibration.rs.  Every workload
// is rendered with unlimited fuel and the fuel it consumed is compared to the
// time it took and the memory it allocated.  For fuel to be a good budget,
// nanoseconds per fuel and bytes per fuel should be roughly the same for
// every workload.
//
// The test is skipped unless MINIJINJA_FUEL_CALIBRATION is set.  Its value
// can be a workload name or group to only measure some of them:
//
//	$ MINIJINJA_FUEL_CALIBRATION=1 go test -run TestFuelCalibration -v .
//	$ MINIJINJA_FUEL_CALIBRATION=seq go test -run TestFuelCalibration -v .

type calibrationWorkload struct {
	group    string
	name     string
	template string
	ctx      func() value.Value
}

func calibrationNoCtx() value.Value {
	return value.FromMap(map[string]value.Value{})
}

func calibrationDataCtx() value.Value {
	items := make([]any, 1000)
	words := make([]any, 1000)
	users := make([]any, 1000)
	mapping := make(map[string]any, 1000)
	for x := 0; x < 1000; x++ {
		items[x] = x
		words[x] = fmt.Sprintf("word%d", x)
		users[x] = map[string]any{
			"name": fmt.Sprintf("user%d", x),
			"age":  x % 90,
			"city": fmt.Sprintf("city%d", x%17),
		}
		mapping[fmt.Sprintf("key%d", x)] = x
	}
	ctx := value.FromAny(map[string]any{
		"items":   items,
		"words":   words,
		"users":   users,
		"mapping": mapping,
		"text":    strings.Repeat("lorem ipsum dolor sit amet ", 400),
		"lines":   strings.Repeat("lorem ipsum dolor sit amet\n", 400),
	})
	m, _ := ctx.AsMap()
	m["iter"] = value.MakeIterable(func() iter.Seq[value.Value] {
		return func(yield func(value.Value) bool) {
			for i := int64(0); i < 1000; i++ {
				if !yield(value.FromInt(i)) {
					return
				}
			}
		}
	})
	return ctx
}

func calibrationAllElementsCtx() value.Value {
	items := []any{}
	for x := 3; x < 200; x++ {
		items = append(items, x)
	}
	return value.FromAny(map[string]any{
		"DEBUG": false,
		"site": map[string]any{
			"nav": []any{
				map[string]any{"url": "/", "is_active": true, "title": "Index"},
				map[string]any{"url": "/doc", "is_active": false, "title": "Docs"},
				map[string]any{"url": "/help", "is_active": false, "title": "Help"},
			},
		},
		"items": items,
	})
}

func calibrationStringHeavyCtx() value.Value {
	recent := []any{}
	for x := 0; x < 8; x++ {
		recent = append(recent, map[string]any{
			"url":      fmt.Sprintf("/posts/%d", x),
			"title":    "Understanding Template Engine Internals & Performance Characteristics",
			"date":     "2026-03-10",
			"featured": x%3 == 0,
		})
	}
	return value.FromAny(map[string]any{
		"post": map[string]any{
			"title":   "the quick brown fox jumps over the lazy dog",
			"date":    "2026-03-14",
			"tags":    []any{"Rust Programming", "Template Engines", "Performance", "Web Development", "Open Source"},
			"summary": "This is a longer summary that contains various characters like <html> tags, \"quotes\", and other things that need escaping. It should be long enough to exercise the truncate filter and HTML escaping paths in a meaningful way for benchmarking purposes.",
			"paragraphs": []any{
				"MiniJinja is a powerful template engine for Rust that implements a large subset of the Jinja2 template language.",
				"It focuses on providing a minimal dependency footprint while still supporting advanced features like template inheritance, macros, and custom filters.",
				"Performance is a key goal, with careful attention paid to minimizing allocations and optimizing hot paths in the rendering pipeline.",
				"The engine supports auto-escaping, making it safe to use with HTML output without worrying about XSS vulnerabilities.",
				"Custom filters and functions can be registered to extend the engine's capabilities for specific use cases.",
			},
			"footer_note": "Originally published on the <em>Rust Blog</em> &mdash; all rights reserved.",
			"author": map[string]any{
				"name":         "Armin Ronacher",
				"recent_posts": recent,
			},
		},
	})
}

func calibrationMacroHeavyCtx() value.Value {
	items := []any{}
	for x := 0; x < 4; x++ {
		items = append(items, map[string]any{
			"title":       fmt.Sprintf("Project %d", x),
			"description": "A powerful Jinja2-compatible template engine for Rust with minimal dependencies and excellent performance characteristics.",
			"featured":    x%2 == 0,
			"metadata":    map[string]any{"language": "Rust", "license": "Apache-2.0", "stars": "2.1k"},
			"actions": []any{
				map[string]any{"url": "/projects/minijinja", "label": "View", "style": "primary"},
				map[string]any{"url": "/projects/minijinja/edit", "label": "Edit", "style": "secondary"},
			},
		})
	}
	return value.FromAny(map[string]any{
		"page": map[string]any{
			"title":       "Dashboard Settings",
			"form_action": "/api/settings/save",
			"items_title": "Your Projects",
		},
		"user": map[string]any{
			"first_name": "Jane",
			"last_name":  "Doe",
			"email":      "jane.doe@example.com",
			"bio":        "Software engineer focused on systems programming and developer tools.",
			"role_options": []any{
				map[string]any{"value": "admin", "label": "Administrator", "selected": false},
				map[string]any{"value": "editor", "label": "Editor", "selected": true},
				map[string]any{"value": "viewer", "label": "Viewer", "selected": false},
			},
		},
		"items": items,
		"notifications": []any{
			map[string]any{"level": "info", "title": "Update Available", "message": "Version 3.0 is ready to install.", "action": map[string]any{"url": "/update", "label": "Install Now"}},
			map[string]any{"level": "warning", "title": "API Quota", "message": "You have used 85% of your monthly API quota."},
		},
	})
}

var calibrationWorkloads = []calibrationWorkload{
	// realistic templates
	{"real", "all_elements", "{% include 'all_elements.html' %}", calibrationAllElementsCtx},
	{"real", "string_heavy", "{% include 'string_heavy.html' %}", calibrationStringHeavyCtx},
	{"real", "macro_heavy", "{% include 'macro_heavy.html' %}", calibrationMacroHeavyCtx},
	{"real", "loop_attrs", "{% for u in users %}<li>{{ u.name }} ({{ u.age }}) from {{ u.city|title }}</li>{% endfor %}", calibrationDataCtx},
	{"real", "loop_if", "{% for x in items %}{% if x is even %}{{ x }}{% else %}-{% endif %}{% endfor %}", calibrationDataCtx},
	// instructions
	{"instr", "empty_loop", "{% for x in range(10000) %}{% endfor %}", calibrationNoCtx},
	{"instr", "arith", "{% for x in range(10000) %}{% set y = x * 2 + 1 %}{% endfor %}", calibrationNoCtx},
	{"instr", "emit_int", "{% for x in range(10000) %}{{ x }}{% endfor %}", calibrationNoCtx},
	// output
	{"output", "emit_text", "{% for x in range(1000) %}{{ text }}{% endfor %}", calibrationDataCtx},
	{"output", "raw_text", "{% for x in range(10000) %}lorem ipsum dolor sit amet lorem ipsum dolor sit amet lorem ipsum dolor sit amet lorem ipsum dolor sit amet{% endfor %}", calibrationNoCtx},
	{"output", "capture", "{% for x in range(1000) %}{% set y %}{{ text }}{% endset %}{% endfor %}", calibrationDataCtx},
	{"output", "escape", "{% autoescape true %}{% for x in range(1000) %}{{ text }}{% endfor %}{% endautoescape %}", calibrationDataCtx},
	// string building
	{"string", "repeat", "{% for x in range(1000) %}{% set y = 'abc' * 1000 %}{% endfor %}", calibrationNoCtx},
	{"string", "concat", "{% for x in range(1000) %}{% set y = text ~ text %}{% endfor %}", calibrationDataCtx},
	{"string", "grow", "{% set ns = namespace(s='') %}{% for x in range(3000) %}{% set ns.s = ns.s ~ 'abcdefgh' %}{% endfor %}", calibrationNoCtx},
	{"string", "upper", "{% for x in range(1000) %}{% set y = text|upper %}{% endfor %}", calibrationDataCtx},
	{"string", "replace", "{% for x in range(1000) %}{% set y = text|replace('o', '00') %}{% endfor %}", calibrationDataCtx},
	{"string", "length", "{% for x in range(1000) %}{% set y = text|length %}{% endfor %}", calibrationDataCtx},
	{"string", "split", "{% for x in range(1000) %}{% set y = text|split %}{% endfor %}", calibrationDataCtx},
	{"string", "lines", "{% for x in range(1000) %}{% set y = lines|lines %}{% endfor %}", calibrationDataCtx},
	{"string", "indent", "{% for x in range(1000) %}{% set y = lines|indent(8) %}{% endfor %}", calibrationDataCtx},
	{"string", "format", "{% for x in range(10000) %}{% set y = '%10s|%-8d|%.3f'|format('a', x, 1.5) %}{% endfor %}", calibrationNoCtx},
	{"string", "in_text", "{% for x in range(1000) %}{% set y = 'xyz' in text %}{% endfor %}", calibrationDataCtx},
	{"string", "slice", "{% for x in range(1000) %}{% set y = text[10:-10] %}{% endfor %}", calibrationDataCtx},
	// sequences
	{"seq", "list", "{% for x in range(100) %}{% set y = items|list %}{% endfor %}", calibrationDataCtx},
	{"seq", "list_iter", "{% for x in range(100) %}{% set y = iter|list %}{% endfor %}", calibrationDataCtx},
	{"seq", "sort", "{% for x in range(100) %}{% set y = words|sort %}{% endfor %}", calibrationDataCtx},
	{"seq", "sort_attr", "{% for x in range(100) %}{% set y = users|sort(attribute='age') %}{% endfor %}", calibrationDataCtx},
	{"seq", "join", "{% for x in range(100) %}{% set y = words|join(', ') %}{% endfor %}", calibrationDataCtx},
	{"seq", "join_ints", "{% for x in range(100) %}{% set y = items|join(', ') %}{% endfor %}", calibrationDataCtx},
	{"seq", "unique", "{% for x in range(100) %}{% set y = words|unique %}{% endfor %}", calibrationDataCtx},
	{"seq", "sum", "{% for x in range(100) %}{% set y = items|sum %}{% endfor %}", calibrationDataCtx},
	{"seq", "max", "{% for x in range(100) %}{% set y = items|max %}{% endfor %}", calibrationDataCtx},
	{"seq", "reverse", "{% for x in range(100) %}{% set y = iter|reverse %}{% endfor %}", calibrationDataCtx},
	{"seq", "last", "{% for x in range(100) %}{% set y = iter|last %}{% endfor %}", calibrationDataCtx},
	{"seq", "map_attr", "{% for x in range(100) %}{% set y = users|map(attribute='name') %}{% endfor %}", calibrationDataCtx},
	{"seq", "map_filter", "{% for x in range(100) %}{% set y = words|map('upper') %}{% endfor %}", calibrationDataCtx},
	{"seq", "select", "{% for x in range(100) %}{% set y = items|select('even') %}{% endfor %}", calibrationDataCtx},
	{"seq", "selectattr", "{% for x in range(100) %}{% set y = users|selectattr('age', 'even') %}{% endfor %}", calibrationDataCtx},
	{"seq", "batch", "{% for x in range(100) %}{% set y = items|batch(3) %}{% endfor %}", calibrationDataCtx},
	{"seq", "slice", "{% for x in range(100) %}{% set y = items|slice(3) %}{% endfor %}", calibrationDataCtx},
	{"seq", "groupby", "{% for x in range(100) %}{% set y = users|groupby('city') %}{% endfor %}", calibrationDataCtx},
	{"seq", "dictsort", "{% for x in range(100) %}{% set y = mapping|dictsort %}{% endfor %}", calibrationDataCtx},
	{"seq", "in_list", "{% for x in range(100) %}{% set y = -1 in items %}{% endfor %}", calibrationDataCtx},
	{"seq", "in_iter", "{% for x in range(100) %}{% set y = -1 in iter %}{% endfor %}", calibrationDataCtx},
	{"seq", "eq", "{% for x in range(100) %}{% set y = items == items|list %}{% endfor %}", calibrationDataCtx},
	{"seq", "tojson", "{% for x in range(100) %}{% set y = users|tojson %}{% endfor %}", calibrationDataCtx},
	{"seq", "emit_list", "{% for x in range(100) %}{{ items }}{% endfor %}", calibrationDataCtx},
	{"seq", "emit_users", "{% for x in range(100) %}{{ users }}{% endfor %}", calibrationDataCtx},
	{"seq", "pprint", "{% for x in range(100) %}{% set y = users|pprint %}{% endfor %}", calibrationDataCtx},
	{"seq", "string", "{% for x in range(100) %}{% set y = items|string %}{% endfor %}", calibrationDataCtx},
	{"seq", "literal", "{% for x in range(10000) %}{% set y = [x, x, x, x] %}{% endfor %}", calibrationNoCtx},
	{"seq", "map_literal", "{% for x in range(10000) %}{% set y = {'a': x, 'b': x} %}{% endfor %}", calibrationNoCtx},
	{"seq", "add", "{% for x in range(100) %}{% set y = (items + items)|list %}{% endfor %}", calibrationDataCtx},
	{"seq", "rev_slice", "{% for x in range(100) %}{% set y = iter[::-1] %}{% endfor %}", calibrationDataCtx},
	{"seq", "index_iter", "{% for x in range(100) %}{% set y = iter[999] %}{% endfor %}", calibrationDataCtx},
	// Go specific
	{"go", "range", "{% for x in range(100) %}{% set y = range(1000) %}{% endfor %}", calibrationNoCtx},
	{"go", "loop_list", "{% for x in range(10) %}{% for y in items %}{% endfor %}{% endfor %}", calibrationDataCtx},
	{"go", "loop_iter", "{% for x in range(10) %}{% for y in iter %}{% endfor %}{% endfor %}", calibrationDataCtx},
}

func calibrationEnv(t *testing.T) *Environment {
	env := NewEnvironment()
	for _, name := range []string{"footer.html", "all_elements.html", "string_heavy.html", "macro_heavy.html"} {
		source, err := os.ReadFile(filepath.Join("..", "benchmarks", "inputs", name))
		if err != nil {
			t.Fatalf("cannot read benchmark input: %v", err)
		}
		if err := env.AddTemplate(name, string(source)); err != nil {
			t.Fatalf("cannot add %s: %v", name, err)
		}
	}
	env.AddFilter("asset_url", func(_ FilterState, val value.Value, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
		return val, nil
	})
	env.AddFunction("current_year", func(_ *State, _ []value.Value, _ map[string]value.Value) (value.Value, error) {
		return value.FromInt(2022), nil
	})
	fuel := uint64(1) << 50
	env.SetFuel(&fuel)
	return env
}

type calibrationMeasurement struct {
	fuel      uint64
	ns        float64
	allocated uint64
}

func measureWorkload(t *testing.T, env *Environment, w calibrationWorkload) calibrationMeasurement {
	tmpl, err := env.TemplateFromString(w.template)
	if err != nil {
		t.Fatalf("%s: %v", w.name, err)
	}
	ctx := w.ctx()

	// warm up and find the fuel
	var fuel uint64
	for i := 0; i < 3; i++ {
		state, err := tmpl.EvalToState(ctx)
		if err != nil {
			t.Fatalf("%s: %v", w.name, err)
		}
		fuel, _, _ = state.FuelLevels()
	}

	// memory
	runtime.GC()
	var before, after runtime.MemStats
	runtime.ReadMemStats(&before)
	if _, err := tmpl.Render(ctx); err != nil {
		t.Fatalf("%s: %v", w.name, err)
	}
	runtime.ReadMemStats(&after)
	allocated := after.TotalAlloc - before.TotalAlloc

	// time.  The fastest sample is used as it's the least affected by other
	// load on the machine.
	samples := make([]float64, 0, 25)
	for i := 0; i < 25; i++ {
		start := time.Now()
		n := 0
		for time.Since(start) < 20*time.Millisecond || n < 3 {
			if _, err := tmpl.Render(ctx); err != nil {
				t.Fatalf("%s: %v", w.name, err)
			}
			n++
		}
		samples = append(samples, float64(time.Since(start).Nanoseconds())/float64(n))
	}
	sort.Float64s(samples)

	return calibrationMeasurement{fuel: fuel, ns: samples[0], allocated: allocated}
}

func TestFuelCalibration(t *testing.T) {
	filter := os.Getenv("MINIJINJA_FUEL_CALIBRATION")
	if filter == "" {
		t.Skip("set MINIJINJA_FUEL_CALIBRATION to run")
	}
	if filter == "1" || filter == "all" {
		filter = ""
	}
	env := calibrationEnv(t)

	// the instruction workloads are the baseline that everything else is
	// compared to.
	var rates []float64
	for _, w := range calibrationWorkloads {
		if w.group == "instr" {
			m := measureWorkload(t, env, w)
			rates = append(rates, m.ns/float64(max(m.fuel, 1)))
		}
	}
	sort.Float64s(rates)
	baseline := rates[len(rates)/2]

	var out strings.Builder
	fmt.Fprintf(&out, "baseline: %.2f ns/fuel\n", baseline)
	fmt.Fprintf(&out, "%-8s %-14s %10s %12s %9s %7s %10s\n", "group", "workload", "fuel", "time", "ns/fuel", "rel", "alloc/fuel")
	for _, w := range calibrationWorkloads {
		if filter != "" && !strings.Contains(w.name, filter) && w.group != filter {
			continue
		}
		m := measureWorkload(t, env, w)
		fuel := float64(max(m.fuel, 1))
		fmt.Fprintf(&out, "%-8s %-14s %10d %10.1fµs %9.2f %7.2f %10.1f\n",
			w.group, w.name, m.fuel, m.ns/1000, m.ns/fuel, m.ns/fuel/baseline, float64(m.allocated)/fuel)
	}
	t.Log("\n" + out.String())
}
