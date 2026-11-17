package checks

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"strings"
	"unicode/utf8"

	"maintainablecheck/internal/report"
)

func checkJSON(file string, source []byte) []report.Finding {
	source = bytes.TrimPrefix(source, []byte{0xef, 0xbb, 0xbf})
	if json.Valid(source) {
		return nil
	}
	var value any
	err := json.Unmarshal(source, &value)
	if err == nil {
		return nil
	}
	offset := len(source)
	var syntax *json.SyntaxError
	if errors.As(err, &syntax) {
		offset = max(0, int(syntax.Offset)-1)
	}
	before := string(source[:min(offset, len(source))])
	line := strings.Count(before, "\n") + 1
	column := utf8.RuneCountInString(before[strings.LastIndex(before, "\n")+1:]) + 1
	return []report.Finding{finding(file, "error", "json/syntax", err.Error(), line, column)}
}

func decodeJSON(source []byte, destination any) error {
	decoder := json.NewDecoder(bytes.NewReader(bytes.TrimPrefix(source, []byte{0xef, 0xbb, 0xbf})))
	if err := decoder.Decode(destination); err != nil {
		return err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return errors.New("trailing JSON content")
	}
	return nil
}
