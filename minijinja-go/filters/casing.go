package filters

import (
	"strings"
	"unicode"
	"unicode/utf8"
)

// Case conversions for the upper, lower, capitalize and title filters.
//
// The unicode package only provides one-to-one case mappings, while Rust
// and Python use the full mappings from SpecialCasing.txt: "ß" uppercases
// to "SS", "ﬁ" to "FI" and "İ" lowercases to "i̇".  They also lowercase a
// capital sigma at the end of a word to a final sigma ("ς").  These helpers
// implement the same conversions as Rust's str::to_uppercase and
// str::to_lowercase.

// upperExpansions holds the uppercase mappings that expand into more than
// one character.  These are the unconditional mappings of SpecialCasing.txt.
var upperExpansions = map[rune]string{
	0x00df: "SS",
	0x0149: "\u02bcN",
	0x01f0: "J\u030c",
	0x0390: "\u0399\u0308\u0301",
	0x03b0: "\u03a5\u0308\u0301",
	0x0587: "\u0535\u0552",
	0x1e96: "H\u0331",
	0x1e97: "T\u0308",
	0x1e98: "W\u030a",
	0x1e99: "Y\u030a",
	0x1e9a: "A\u02be",
	0x1f50: "\u03a5\u0313",
	0x1f52: "\u03a5\u0313\u0300",
	0x1f54: "\u03a5\u0313\u0301",
	0x1f56: "\u03a5\u0313\u0342",
	0x1f80: "\u1f08\u0399",
	0x1f81: "\u1f09\u0399",
	0x1f82: "\u1f0a\u0399",
	0x1f83: "\u1f0b\u0399",
	0x1f84: "\u1f0c\u0399",
	0x1f85: "\u1f0d\u0399",
	0x1f86: "\u1f0e\u0399",
	0x1f87: "\u1f0f\u0399",
	0x1f88: "\u1f08\u0399",
	0x1f89: "\u1f09\u0399",
	0x1f8a: "\u1f0a\u0399",
	0x1f8b: "\u1f0b\u0399",
	0x1f8c: "\u1f0c\u0399",
	0x1f8d: "\u1f0d\u0399",
	0x1f8e: "\u1f0e\u0399",
	0x1f8f: "\u1f0f\u0399",
	0x1f90: "\u1f28\u0399",
	0x1f91: "\u1f29\u0399",
	0x1f92: "\u1f2a\u0399",
	0x1f93: "\u1f2b\u0399",
	0x1f94: "\u1f2c\u0399",
	0x1f95: "\u1f2d\u0399",
	0x1f96: "\u1f2e\u0399",
	0x1f97: "\u1f2f\u0399",
	0x1f98: "\u1f28\u0399",
	0x1f99: "\u1f29\u0399",
	0x1f9a: "\u1f2a\u0399",
	0x1f9b: "\u1f2b\u0399",
	0x1f9c: "\u1f2c\u0399",
	0x1f9d: "\u1f2d\u0399",
	0x1f9e: "\u1f2e\u0399",
	0x1f9f: "\u1f2f\u0399",
	0x1fa0: "\u1f68\u0399",
	0x1fa1: "\u1f69\u0399",
	0x1fa2: "\u1f6a\u0399",
	0x1fa3: "\u1f6b\u0399",
	0x1fa4: "\u1f6c\u0399",
	0x1fa5: "\u1f6d\u0399",
	0x1fa6: "\u1f6e\u0399",
	0x1fa7: "\u1f6f\u0399",
	0x1fa8: "\u1f68\u0399",
	0x1fa9: "\u1f69\u0399",
	0x1faa: "\u1f6a\u0399",
	0x1fab: "\u1f6b\u0399",
	0x1fac: "\u1f6c\u0399",
	0x1fad: "\u1f6d\u0399",
	0x1fae: "\u1f6e\u0399",
	0x1faf: "\u1f6f\u0399",
	0x1fb2: "\u1fba\u0399",
	0x1fb3: "\u0391\u0399",
	0x1fb4: "\u0386\u0399",
	0x1fb6: "\u0391\u0342",
	0x1fb7: "\u0391\u0342\u0399",
	0x1fbc: "\u0391\u0399",
	0x1fc2: "\u1fca\u0399",
	0x1fc3: "\u0397\u0399",
	0x1fc4: "\u0389\u0399",
	0x1fc6: "\u0397\u0342",
	0x1fc7: "\u0397\u0342\u0399",
	0x1fcc: "\u0397\u0399",
	0x1fd2: "\u0399\u0308\u0300",
	0x1fd3: "\u0399\u0308\u0301",
	0x1fd6: "\u0399\u0342",
	0x1fd7: "\u0399\u0308\u0342",
	0x1fe2: "\u03a5\u0308\u0300",
	0x1fe3: "\u03a5\u0308\u0301",
	0x1fe4: "\u03a1\u0313",
	0x1fe6: "\u03a5\u0342",
	0x1fe7: "\u03a5\u0308\u0342",
	0x1ff2: "\u1ffa\u0399",
	0x1ff3: "\u03a9\u0399",
	0x1ff4: "\u038f\u0399",
	0x1ff6: "\u03a9\u0342",
	0x1ff7: "\u03a9\u0342\u0399",
	0x1ffc: "\u03a9\u0399",
	0xfb00: "FF",
	0xfb01: "FI",
	0xfb02: "FL",
	0xfb03: "FFI",
	0xfb04: "FFL",
	0xfb05: "ST",
	0xfb06: "ST",
	0xfb13: "\u0544\u0546",
	0xfb14: "\u0544\u0535",
	0xfb15: "\u0544\u053b",
	0xfb16: "\u054e\u0546",
	0xfb17: "\u0544\u053d",
}

// writeUpper appends the uppercase form of r.
func writeUpper(b *strings.Builder, r rune) {
	if r < utf8.RuneSelf {
		if 'a' <= r && r <= 'z' {
			r -= 'a' - 'A'
		}
		b.WriteByte(byte(r))
	} else if expansion, ok := upperExpansion(r); ok {
		b.WriteString(expansion)
	} else {
		b.WriteRune(unicode.ToUpper(r))
	}
}

// upperExpansion returns the uppercase form of r if it expands into more
// than one character.
func upperExpansion(r rune) (string, bool) {
	// all expansions are in these ranges, which avoids a map lookup for
	// most characters.
	if r != 0xdf && (r < 0x149 || r > 0x587) && (r < 0x1e96 || r > 0x1ffc) && (r < 0xfb00 || r > 0xfb17) {
		return "", false
	}
	expansion, ok := upperExpansions[r]
	return expansion, ok
}

// writeTitle appends the titlecase form of r.  For most characters this is
// the same as the uppercase form.  The exceptions are taken from the Unicode
// character database.
func writeTitle(b *strings.Builder, r rune) {
	switch {
	// the digraphs DŽ, LJ, NJ and DZ have dedicated titlecase forms
	case r >= 0x1c4 && r <= 0x1cc:
		r = r - (r-0x1c4)%3 + 1
	case r >= 0x1f1 && r <= 0x1f3:
		r = 0x1f2
	// Georgian Mkhedruli letters are their own titlecase form
	case r >= 0x10d0 && r <= 0x10fa, r >= 0x10fd && r <= 0x10ff:
	// Greek letters with ypogegrammeni titlecase to prosgegrammeni
	case r >= 0x1f80 && r <= 0x1faf:
		r |= 0x08
	case r == 0x1fb3 || r == 0x1fbc:
		r = 0x1fbc
	case r == 0x1fc3 || r == 0x1fcc:
		r = 0x1fcc
	case r == 0x1ff3 || r == 0x1ffc:
		r = 0x1ffc
	default:
		if expansion, ok := titleExpansions[r]; ok {
			b.WriteString(expansion)
		} else {
			writeUpper(b, r)
		}
		return
	}
	b.WriteRune(r)
}

// titleExpansions holds the titlecase mappings that expand into more than
// one character.
var titleExpansions = map[rune]string{
	0x00df: "Ss",
	0x0587: "\u0535\u0582",
	0x1fb2: "\u1fba\u0345",
	0x1fb4: "\u0386\u0345",
	0x1fb7: "\u0391\u0342\u0345",
	0x1fc2: "\u1fca\u0345",
	0x1fc4: "\u0389\u0345",
	0x1fc7: "\u0397\u0342\u0345",
	0x1ff2: "\u1ffa\u0345",
	0x1ff4: "\u038f\u0345",
	0x1ff7: "\u03a9\u0342\u0345",
	0xfb00: "Ff",
	0xfb01: "Fi",
	0xfb02: "Fl",
	0xfb03: "Ffi",
	0xfb04: "Ffl",
	0xfb05: "St",
	0xfb06: "St",
	0xfb13: "\u0544\u0576",
	0xfb14: "\u0544\u0565",
	0xfb15: "\u0544\u056b",
	0xfb16: "\u054e\u0576",
	0xfb17: "\u0544\u056d",
}

// toUpper converts s to uppercase like Rust's str::to_uppercase.
func toUpper(s string) string {
	if isASCII(s) {
		return strings.ToUpper(s)
	}
	var b strings.Builder
	b.Grow(len(s))
	for _, r := range s {
		writeUpper(&b, r)
	}
	return b.String()
}

// toLower converts s to lowercase like Rust's str::to_lowercase.
func toLower(s string) string {
	if !strings.Contains(s, "\u03a3") && !strings.Contains(s, "\u0130") {
		return strings.ToLower(s)
	}
	var b strings.Builder
	b.Grow(len(s) + 1)
	writeLower(&b, s, 0)
	return b.String()
}

// writeLowerTail appends s[start:] lowercased.  s[:start] is only used as
// context to find out if a capital sigma is at the end of a word.
func writeLowerTail(b *strings.Builder, s string, start int) {
	tail := s[start:]
	if !isASCII(tail) {
		writeLower(b, s, start)
		return
	}
	for i := 0; i < len(tail); i++ {
		c := tail[i]
		if 'A' <= c && c <= 'Z' {
			c += 'a' - 'A'
		}
		b.WriteByte(c)
	}
}

func writeLower(b *strings.Builder, s string, start int) {
	for i, r := range s[start:] {
		switch r {
		case 'Σ':
			if isFinalSigma(s, start+i) {
				b.WriteRune('ς')
			} else {
				b.WriteRune('σ')
			}
		default:
			writeLowerRune(b, r)
		}
	}
}

// writeLowerRune appends the lowercase form of r, which must not be a
// capital sigma.
func writeLowerRune(b *strings.Builder, r rune) {
	if r == 'İ' {
		b.WriteString("i\u0307")
	} else {
		b.WriteRune(unicode.ToLower(r))
	}
}

// isFinalSigma reports whether the capital sigma at s[i] lowercases to a
// final sigma.  This implements the Final_Sigma condition of the Unicode
// standard: it must follow a cased letter and must not be followed by one,
// ignoring case-ignorable characters in between.
func isFinalSigma(s string, i int) bool {
	before := s[:i]
	for len(before) > 0 {
		r, size := utf8.DecodeLastRuneInString(before)
		if !isCaseIgnorable(r) {
			if !isCased(r) {
				return false
			}
			break
		}
		before = before[:len(before)-size]
	}
	if len(before) == 0 {
		return false
	}
	for _, r := range s[i+len("Σ"):] {
		if !isCaseIgnorable(r) {
			return !isCased(r)
		}
	}
	return true
}

// isCased reports whether r has the Cased property.
func isCased(r rune) bool {
	return unicode.IsUpper(r) || unicode.IsLower(r) || unicode.IsTitle(r) ||
		unicode.In(r, unicode.Other_Lowercase, unicode.Other_Uppercase)
}

// isCaseIgnorable reports whether r has the Case_Ignorable property.
func isCaseIgnorable(r rune) bool {
	switch r {
	// characters with the word break property MidLetter, MidNumLet or
	// Single_Quote
	case '\'', '.', ':', 0xb7, 0x387, 0x55f, 0x5f4, 0x2018, 0x2019, 0x2024,
		0x2027, 0xfe13, 0xfe52, 0xfe55, 0xff07, 0xff0e, 0xff1a:
		return true
	}
	return unicode.In(r, unicode.Mn, unicode.Me, unicode.Cf, unicode.Lm, unicode.Sk)
}

func isASCII(s string) bool {
	for i := 0; i < len(s); i++ {
		if s[i] >= utf8.RuneSelf {
			return false
		}
	}
	return true
}
