import asyncio
import pytest


# rqtk: verifies VA-PY-01
def test_plain():
    assert 1 + 1 == 2


# rqtk: verifies VA-PY-02
@pytest.mark.parametrize(
    "n",
    [1, 2, 3],
)
def test_parametrized(n):
    assert n > 0


# rqtk: verifies VA-PY-03 case "pos"
# rqtk: verifies VA-PY-04 case "neg"
@pytest.mark.parametrize("sign", ["pos", "neg"])
def test_sign(sign):
    assert sign == "pos"


class TestGroup:
    # rqtk: verifies VA-PY-05
    def test_in_class(self):
        assert True


# rqtk: verifies VA-PY-06
@pytest.mark.skip(reason="not yet")
def test_skipped():
    pass


# rqtk: verifies VA-PY-07
async def test_async_def():
    assert True


# rqtk: verifies VA-PY-08
def test_duplicate_name():
    assert True
