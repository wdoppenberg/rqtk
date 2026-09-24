package corpus

import "testing"

// rqtk: verifies VA-GO-01
func TestPlain(t *testing.T) {}

// rqtk: verifies VA-GO-02
func TestWithSubtests(t *testing.T) {
	for _, name := range []string{"a", "b"} {
		t.Run(name, func(t *testing.T) {})
	}
}

func TestTable(t *testing.T) {
	cases := []struct {
		name string
		ok   bool
	}{
		// rqtk: verifies VA-GO-03 case "flipped byte is corrupt"
		{"flipped byte is corrupt", true},
		// rqtk: verifies VA-GO-04 case "intact file passes"
		{"intact file passes", false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if !tc.ok {
				t.Fatal("broken")
			}
		})
	}
}

type suite struct{}

// rqtk: verifies VA-GO-05
func (s *suite) TestMethodStyle(t *testing.T) {}

func TestSuite(t *testing.T) { (&suite{}).TestMethodStyle(t) }

// rqtk: verifies VA-GO-06
func TestSkipped(t *testing.T) { t.Skip("later") }
