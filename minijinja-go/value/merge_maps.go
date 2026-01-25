package value

import "sort"

// MergeMaps merges multiple map-like values into a single lazy map object.
//
// Later values override earlier ones when keys overlap. Non-map values are
// ignored for enumeration, but attribute lookups are forwarded to any objects
// that implement map-like access.
//
// This mirrors the behavior of MiniJinja's context merging in Rust.
func MergeMaps(sources ...Value) Value {
	if len(sources) == 1 {
		return sources[0]
	}
	return FromObject(&mergedMap{sources: sources})
}

type mergedMap struct {
	sources []Value
}

func (m *mergedMap) ObjectRepr() ObjectRepr {
	return ObjectReprMap
}

func (m *mergedMap) ObjectLen() int {
	return len(m.Keys())
}

func (m *mergedMap) Keys() []string {
	keySet := make(map[string]struct{})
	for _, src := range m.sources {
		for _, key := range keysForValue(src) {
			keySet[key] = struct{}{}
		}
	}
	keys := make([]string, 0, len(keySet))
	for key := range keySet {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}

func (m *mergedMap) GetAttr(name string) Value {
	for i := len(m.sources) - 1; i >= 0; i-- {
		val := m.sources[i].GetAttr(name)
		if !val.IsUndefined() {
			return val
		}
	}
	return Undefined()
}

func (m *mergedMap) Map() map[string]Value {
	keys := m.Keys()
	result := make(map[string]Value, len(keys))
	for _, key := range keys {
		result[key] = m.GetAttr(key)
	}
	return result
}

func keysForValue(v Value) []string {
	if m, ok := v.AsMap(); ok {
		keys := make([]string, 0, len(m))
		for key := range m {
			keys = append(keys, key)
		}
		return keys
	}
	if obj, ok := v.AsObject(); ok {
		if m, ok := obj.(MapObject); ok {
			return m.Keys()
		}
	}
	return nil
}
