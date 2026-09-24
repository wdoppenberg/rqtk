package dev.corpus;

import static org.junit.jupiter.api.Assertions.*;

import org.junit.jupiter.api.Disabled;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;

class ShapesTest {
    // rqtk: verifies VA-JV-01
    @Test
    void plain() {
        assertTrue(true);
    }

    // rqtk: verifies VA-JV-02
    @ParameterizedTest(name = "{0} + {1} = {2}")
    @CsvSource({"1,1,2", "2,2,4"})
    void parameterized(int a, int b, int sum) {
        assertEquals(sum, a + b);
    }

    // rqtk: verifies VA-JV-03
    @Test
    @DisplayName("a display name")
    public void withDisplayName() {
        assertTrue(true);
    }

    @Nested
    class Inner {
        // rqtk: verifies VA-JV-04
        @Test
        public void nestedTest() {
            assertTrue(true);
        }
    }

    // rqtk: verifies VA-JV-05
    @Test
    @Disabled("later")
    void disabled() {}

    // rqtk: verifies VA-JV-06
    @Test
    void sharedName() {
        assertTrue(true);
    }
}
