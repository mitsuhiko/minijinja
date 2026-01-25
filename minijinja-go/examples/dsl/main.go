// Example: dsl
//
// This example demonstrates a small query DSL using custom objects and
// method calls, ported from the Rust example.
package main

import (
	"fmt"
	"log"
	"os"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

type Query struct {
	table   string
	filters map[string]value.Value
	limit   *int64
	offset  *int64
}

func NewQuery(table string) *Query {
	return &Query{
		table:   table,
		filters: make(map[string]value.Value),
	}
}

func (q *Query) clone() *Query {
	copyFilters := make(map[string]value.Value, len(q.filters))
	for key, val := range q.filters {
		copyFilters[key] = val
	}

	clone := &Query{
		table:   q.table,
		filters: copyFilters,
		limit:   q.limit,
		offset:  q.offset,
	}
	return clone
}

func (q *Query) Filter(kwargs map[string]value.Value) *Query {
	rv := q.clone()
	for key, val := range kwargs {
		rv.filters[key] = val
	}
	return rv
}

func (q *Query) Limit(count int64) *Query {
	rv := q.clone()
	rv.limit = &count
	return rv
}

func (q *Query) Offset(count int64) *Query {
	rv := q.clone()
	rv.offset = &count
	return rv
}

func (q *Query) GetAttr(name string) value.Value {
	return value.Undefined()
}

func (q *Query) CallMethod(state value.State, name string, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	switch name {
	case "filter":
		if len(args) > 0 {
			return value.Undefined(), fmt.Errorf("filter takes only keyword arguments")
		}
		return value.FromObject(q.Filter(kwargs)), nil
	case "limit":
		if len(args) != 1 {
			return value.Undefined(), fmt.Errorf("limit takes exactly one argument")
		}
		count, ok := args[0].AsInt()
		if !ok {
			return value.Undefined(), fmt.Errorf("limit argument must be integer")
		}
		return value.FromObject(q.Limit(count)), nil
	case "offset":
		if len(args) != 1 {
			return value.Undefined(), fmt.Errorf("offset takes exactly one argument")
		}
		count, ok := args[0].AsInt()
		if !ok {
			return value.Undefined(), fmt.Errorf("offset argument must be integer")
		}
		return value.FromObject(q.Offset(count)), nil
	default:
		return value.Undefined(), value.ErrUnknownMethod
	}
}

func (q *Query) String() string {
	return fmt.Sprintf("<Query table=%q>", q.table)
}

func asQuery(val value.Value) (*Query, bool) {
	obj, ok := val.AsObject()
	if !ok {
		return nil, false
	}
	q, ok := obj.(*Query)
	return q, ok
}

func main() {
	env := minijinja.NewEnvironment()

	env.AddFunction("query", func(state *minijinja.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		if len(args) != 1 {
			return value.Undefined(), fmt.Errorf("query expects one table name")
		}
		table, ok := args[0].AsString()
		if !ok {
			return value.Undefined(), fmt.Errorf("query expects table name as string")
		}
		return value.FromObject(NewQuery(table)), nil
	})

	env.AddFilter("filter", func(state minijinja.FilterState, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		q, ok := asQuery(val)
		if !ok {
			return value.Undefined(), fmt.Errorf("filter expects a query object")
		}
		return value.FromObject(q.Filter(kwargs)), nil
	})

	env.AddFilter("limit", func(state minijinja.FilterState, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		q, ok := asQuery(val)
		if !ok {
			return value.Undefined(), fmt.Errorf("limit expects a query object")
		}
		if len(args) != 1 {
			return value.Undefined(), fmt.Errorf("limit expects one argument")
		}
		count, ok := args[0].AsInt()
		if !ok {
			return value.Undefined(), fmt.Errorf("limit argument must be integer")
		}
		return value.FromObject(q.Limit(count)), nil
	})

	env.AddFilter("offset", func(state minijinja.FilterState, val value.Value, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
		q, ok := asQuery(val)
		if !ok {
			return value.Undefined(), fmt.Errorf("offset expects a query object")
		}
		if len(args) != 1 {
			return value.Undefined(), fmt.Errorf("offset expects one argument")
		}
		count, ok := args[0].AsInt()
		if !ok {
			return value.Undefined(), fmt.Errorf("offset argument must be integer")
		}
		return value.FromObject(q.Offset(count)), nil
	})

	expr := "query('my_table').filter(is_active=true)"
	if len(os.Args) > 1 {
		expr = os.Args[1]
	} else {
		fmt.Fprintln(os.Stderr, "no filter provided, using default one")
	}

	fmt.Printf("filter: %s\n", expr)

	templateSource := fmt.Sprintf("{%% set result = %s %%}", expr)
	tmpl, err := env.TemplateFromNamedString("dsl.expr", templateSource)
	if err != nil {
		log.Fatal(err)
	}

	state, err := tmpl.EvalToState(nil)
	if err != nil {
		log.Fatal(err)
	}

	result := state.Lookup("result")
	fmt.Printf("result: %s\n", result.Repr())
}
