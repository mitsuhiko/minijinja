// Example: state-temps
//
// This example demonstrates how to use state temps to cache translation data
// between repeated calls during rendering.
package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/value"
)

const templateSource = `{{ translate('GREETING') }}, {{ username }}!
{{ translate('GOODBYE') }}!
`

var exampleDir string

func translate(state *minijinja.State, args []value.Value, kwargs map[string]value.Value) (value.Value, error) {
	if len(args) != 1 {
		return value.Undefined(), minijinja.NewError(minijinja.ErrMissingArgument, "translate expects a key")
	}

	key, ok := args[0].AsString()
	if !ok {
		return value.Undefined(), minijinja.NewError(minijinja.ErrInvalidOperation, "translate expects a string key")
	}

	lang := "en"
	if langValue, ok := state.Lookup("LANG").AsString(); ok && langValue != "" {
		lang = langValue
	}

	cacheKey := fmt.Sprintf("translation-cache:%s", lang)
	if cached, ok := state.GetTemp(cacheKey); ok {
		if translations, ok := cached.AsMap(); ok {
			if translated, ok := translations[key]; ok {
				return translated, nil
			}
			return value.Undefined(), nil
		}
	}

	translations, err := loadTranslations(exampleDir, lang)
	if err != nil {
		return value.Undefined(), err
	}

	state.SetTemp(cacheKey, value.FromAny(translations))
	if translated, ok := translations[key]; ok {
		return value.FromString(translated), nil
	}
	return value.Undefined(), nil
}

func main() {
	var err error
	exampleDir, err = findExampleDir()
	if err != nil {
		log.Fatal(err)
	}

	env := minijinja.NewEnvironment()
	env.AddFunction("translate", translate)

	tmpl, err := env.TemplateFromString(templateSource)
	if err != nil {
		log.Fatal(err)
	}

	result, err := tmpl.Render(map[string]any{
		"LANG":     langFromEnv(),
		"username": "Peter",
	})
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}

func loadTranslations(baseDir, lang string) (map[string]string, error) {
	path := filepath.Join(baseDir, "src", fmt.Sprintf("%s.txt", lang))
	contents, err := os.ReadFile(path)
	if err != nil {
		return nil, minijinja.NewError(minijinja.ErrInvalidOperation, "could not read translation file")
	}

	translations := make(map[string]string)
	for _, line := range strings.Split(string(contents), "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		key, val, ok := strings.Cut(line, "=")
		if !ok {
			continue
		}
		translations[key] = val
	}
	return translations, nil
}

func langFromEnv() string {
	lang := os.Getenv("LANG")
	if lang == "" {
		return "en"
	}
	lang = strings.Split(lang, "_")[0]
	if lang == "" {
		return "en"
	}
	return lang
}

func findExampleDir() (string, error) {
	candidates := []string{
		".",
		filepath.Join(".", "examples", "state_temps"),
	}
	for _, candidate := range candidates {
		if _, err := os.Stat(filepath.Join(candidate, "src", "en.txt")); err == nil {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("could not locate state-temps example files")
}
