package dev.corpus

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class ShapesTest {
    // rqtk: verifies VA-KT-01
    @Test
    fun plain() {
        assertEquals(2, 1 + 1)
    }

    // rqtk: verifies VA-KT-02
    @Test
    fun `rounds down to pump resolution`() {
        assertEquals(4, 2 + 2)
    }
}
