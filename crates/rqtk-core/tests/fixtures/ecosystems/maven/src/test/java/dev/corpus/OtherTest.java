package dev.corpus;

import static org.junit.jupiter.api.Assertions.*;
import org.junit.jupiter.api.Test;

class OtherTest {
    // Unrelated and failing, with the same name as a linked test in ShapesTest.
    @Test
    void sharedName() {
        fail("unrelated");
    }
}
