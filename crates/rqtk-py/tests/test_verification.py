import pytest

rqtk = pytest.importorskip("rqtk")


def test_verifies_sets_doc_and_marker_for_known_activity():
    @rqtk.verifies("VA-SYS-001-01")
    def sample():
        return 42

    assert sample() == 42
    assert getattr(sample, "__rqtk_verifies__", None) == "VA-SYS-001-01"
    assert sample.__doc__ is not None
    assert "**Verifies**" in sample.__doc__
    assert "**Activity** `VA-SYS-001-01`" in sample.__doc__


def test_verifies_raises_for_unknown_activity():
    with pytest.raises(
        ValueError,
        match=r"verification activity `VA-DOES-NOT-EXIST` not found",
    ):
        rqtk.verifies("VA-DOES-NOT-EXIST")


def test_verifies_accepts_a_case():
    @rqtk.verifies("VA-SYS-001-01", case="neg")
    def sample():
        return 1

    assert sample.__rqtk_case__ == "neg"
