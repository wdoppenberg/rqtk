#include <gtest/gtest.h>

// rqtk: verifies VA-CPP-01
TEST(Shapes, Plain) { EXPECT_EQ(1 + 1, 2); }

class Fixture : public ::testing::Test {};

// rqtk: verifies VA-CPP-02
TEST_F(Fixture, UsesFixture) { EXPECT_TRUE(true); }

class Params : public ::testing::TestWithParam<int> {};

// rqtk: verifies VA-CPP-03
TEST_P(Params, IsPositive) { EXPECT_GT(GetParam(), 0); }
INSTANTIATE_TEST_SUITE_P(Values, Params, ::testing::Values(1, 2));

// rqtk: verifies VA-CPP-04
TEST(Shapes, Fails) { EXPECT_EQ(1, 2); }

// rqtk: verifies VA-CPP-05
TEST(Shapes, DISABLED_Later) {}

// Unrelated and failing, with the same test name as a linked one in another suite.
TEST(Other, Plain) { EXPECT_EQ(1, 2); }
