// Example: generate-yaml
//
// This example demonstrates using custom delimiters to generate YAML files.
// The ${{ ... }} syntax is used for variables to avoid conflicts with
// YAML syntax. This is similar to GitHub Actions workflow syntax.
package main

import (
	"fmt"
	"log"
	"os"
	"sort"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/syntax"
)

const yamlTemplate = `env: ${{ env }}
title: ${{ title }}
skip: ${{ true }}
run: ${{ ["bash", "./script.sh"] }}
yaml_value: ${{ yaml|safe }}
`

func main() {
	env := minijinja.NewEnvironment()

	// Configure GitHub Actions-style syntax for YAML
	env.SetSyntax(syntax.SyntaxConfig{
		BlockStart:   "{%",
		BlockEnd:     "%}",
		VarStart:     "${{",
		VarEnd:       "}}",
		CommentStart: "{#",
		CommentEnd:   "#}",
	})

	err := env.AddTemplate("template.yml", yamlTemplate)
	if err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("template.yml")
	if err != nil {
		log.Fatal(err)
	}

	// Collect environment variables into a sorted map
	envVars := make(map[string]string)
	for _, e := range os.Environ() {
		for i := 0; i < len(e); i++ {
			if e[i] == '=' {
				envVars[e[:i]] = e[i+1:]
				break
			}
		}
	}

	// Sort keys for consistent output
	keys := make([]string, 0, len(envVars))
	for k := range envVars {
		keys = append(keys, k)
	}
	sort.Strings(keys)

	// Only include a few env vars for the example
	limitedEnv := make(map[string]string)
	count := 0
	for _, k := range keys {
		if count >= 3 {
			break
		}
		limitedEnv[k] = envVars[k]
		count++
	}

	result, err := tmpl.Render(map[string]any{
		"env":   limitedEnv,
		"title": "Hello World!",
		"yaml":  "[1, 2, 3]",
	})
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}
