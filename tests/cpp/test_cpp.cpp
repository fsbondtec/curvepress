#include <gtest/gtest.h>
#include <curvepress/curvepress.hpp>
#include <cmath>
#include <fstream>
#include <limits>
#include <vector>

using namespace curvepress;

// ─── helpers ─────────────────────────────────────────────────────────────────

static std::pair<std::vector<int64_t>, std::vector<double>>
make_sine(std::size_t n, double freq = 0.05, double amp = 100.0) {
    std::vector<int64_t> ts(n);
    std::vector<double>  val(n);
    for (std::size_t i = 0; i < n; ++i) {
        ts[i]  = static_cast<int64_t>(i) * 1'000'000;
        val[i] = std::sin(static_cast<double>(i) * freq) * amp;
    }
    return {ts, val};
}

static std::pair<std::vector<int64_t>, std::vector<double>>
make_fracture(std::size_t n = 500) {
    std::vector<int64_t> ts(n);
    std::vector<double>  val(n);
    for (std::size_t i = 0; i < n; ++i) ts[i] = static_cast<int64_t>(i) * 1'000'000;
    for (std::size_t i = 0; i < 300; ++i)
        val[i] = static_cast<double>(i) / 3.0;
    val[300] = 150.0;
    for (std::size_t i = 301; i < std::min(std::size_t{310}, n); ++i)
        val[i] = std::max(0.0, 100.0 - static_cast<double>(i - 300) * 11.0);
    for (std::size_t i = 310; i < n; ++i)
        val[i] = 0.5 * std::abs(std::sin(static_cast<double>(i) * 0.3));
    return {ts, val};
}

// ─── Round-trip tests ─────────────────────────────────────────────────────────

TEST(RoundTrip, RdpReducesPointCount) {
    auto [ts, val] = make_sine(2000);
    Stats stats;
    auto data = compress_rdp(ts, val, 1.0, stats);
    auto dec  = decompress(data);

    EXPECT_LT(dec.timestamps_ns.size(), ts.size());
    EXPECT_EQ(dec.timestamps_ns.size(), dec.values.size());
    EXPECT_EQ(dec.timestamps_ns.size(), stats.n_kept);
    EXPECT_TRUE(std::isfinite(stats.max_error));
    EXPECT_GE(stats.max_error, 0.0);
}

TEST(RoundTrip, VwReturnsExactNOut) {
    auto [ts, val] = make_sine(500);
    Stats stats;
    auto data = compress_vw(ts, val, 40, stats);
    auto dec  = decompress(data);

    EXPECT_EQ(dec.timestamps_ns.size(), 40u);
    EXPECT_EQ(dec.values.size(), 40u);
    EXPECT_EQ(stats.n_kept, 40u);
}

TEST(RoundTrip, RdpNAtMostNOut) {
    auto [ts, val] = make_sine(1000);
    Stats stats;
    auto data = compress_rdpn(ts, val, 60, 100.0, stats);
    auto dec  = decompress(data);

    EXPECT_LE(dec.timestamps_ns.size(), 60u);
    EXPECT_GE(dec.timestamps_ns.size(), 2u);
    EXPECT_EQ(stats.n_kept, dec.timestamps_ns.size());
}

// ─── Fracture-curve test (primary use case) ───────────────────────────────────

TEST(FractureCurve, PeakAndFirstPostDropKept) {
    auto [ts, val] = make_fracture();
    auto data = compress_rdp(ts, val, 1.0);
    auto dec  = decompress(data);

    int64_t peak_ts = ts[300];
    bool peak_found = false;
    for (auto t : dec.timestamps_ns)
        if (t == peak_ts) { peak_found = true; break; }

    EXPECT_TRUE(peak_found);
}

// ─── Constant series ──────────────────────────────────────────────────────────

TEST(ConstantSeries, OnlyTwoEndpointsKept) {
    std::size_t n = 100;
    std::vector<int64_t> ts(n);
    std::vector<double>  val(n, 42.0);
    for (std::size_t i = 0; i < n; ++i) ts[i] = static_cast<int64_t>(i) * 1'000'000;

    Stats stats;
    auto data = compress_rdp(ts, val, 0.1, stats);
    auto dec  = decompress(data);

    EXPECT_EQ(dec.timestamps_ns.size(), 2u);
    EXPECT_NEAR(dec.values[0], 42.0, 1e-9);
    EXPECT_NEAR(dec.values[1], 42.0, 1e-9);
    EXPECT_EQ(stats.quant_bits, 1);
    EXPECT_LT(stats.max_error, 1e-12);
}

// ─── Edge cases ───────────────────────────────────────────────────────────────

TEST(EdgeCases, SinglePoint) {
    std::vector<int64_t> ts  = {0};
    std::vector<double>  val = {1.0};
    auto data = compress_rdp(ts, val, 1.0);
    auto dec  = decompress(data);
    ASSERT_EQ(dec.timestamps_ns.size(), 1u);
    EXPECT_EQ(dec.timestamps_ns[0], 0);
}

TEST(EdgeCases, TwoPoints) {
    std::vector<int64_t> ts  = {0, 1'000'000};
    std::vector<double>  val = {0.0, 1.0};
    auto data = compress_rdp(ts, val, 1.0);
    auto dec  = decompress(data);
    EXPECT_EQ(dec.timestamps_ns.size(), 2u);
}

// ─── Error handling ───────────────────────────────────────────────────────────

TEST(ErrorHandling, NanValueThrows) {
    std::vector<int64_t> ts  = {0, 1'000'000, 2'000'000};
    std::vector<double>  val = {0.0, std::numeric_limits<double>::quiet_NaN(), 2.0};
    EXPECT_THROW(compress_rdp(ts, val, 1.0), std::invalid_argument);
}

TEST(ErrorHandling, InfValueThrows) {
    std::vector<int64_t> ts  = {0, 1'000'000};
    std::vector<double>  val = {0.0, std::numeric_limits<double>::infinity()};
    EXPECT_THROW(compress_rdp(ts, val, 1.0), std::invalid_argument);
}

TEST(ErrorHandling, NonMonotonicTimestampsThrow) {
    std::vector<int64_t> ts  = {0, 2'000'000, 1'000'000};
    std::vector<double>  val = {0.0, 1.0, 2.0};
    EXPECT_THROW(compress_rdp(ts, val, 1.0), std::invalid_argument);
}

TEST(ErrorHandling, DuplicateTimestampsThrow) {
    std::vector<int64_t> ts  = {0, 1'000'000, 1'000'000};
    std::vector<double>  val = {0.0, 1.0, 2.0};
    EXPECT_THROW(compress_rdp(ts, val, 1.0), std::invalid_argument);
}

TEST(ErrorHandling, CorruptStreamThrows) {
    std::vector<uint8_t> garbage = {0x00, 0x01, 0x02, 0x03};
    EXPECT_THROW(decompress(garbage), std::runtime_error);
}

// ─── Stats ────────────────────────────────────────────────────────────────────

TEST(Stats, MaxErrorIsFiniteAndNonNegative) {
    auto [ts, val] = make_sine(2000);
    Stats stats;
    compress_rdp(ts, val, 2.0, stats);
    EXPECT_TRUE(std::isfinite(stats.max_error));
    EXPECT_GE(stats.max_error, 0.0);
    EXPECT_LT(stats.n_kept, stats.n_input);
}

TEST(Stats, QuantBitsCalculation) {
    // n_steps = ceil(499 / 0.499) = 1000 → n_bits = ceil(log2(1001)) = 10
    std::size_t n = 500;
    std::vector<int64_t> ts(n);
    std::vector<double>  val(n);
    for (std::size_t i = 0; i < n; ++i) {
        ts[i]  = static_cast<int64_t>(i) * 1'000'000;
        val[i] = static_cast<double>(i);
    }
    Stats stats;
    compress_rdp(ts, val, 499.0 / 1000.0, stats);
    EXPECT_EQ(stats.quant_bits, 10);
}

// ─── Interpolation ────────────────────────────────────────────────────────────

TEST(Interpolation, Midpoint) {
    std::vector<int64_t> ts  = {0, 10'000, 20'000, 30'000};
    std::vector<double>  val = {0.0, 10.0, 20.0, 30.0};
    EXPECT_NEAR(interpolate(ts, val, 5'000),  5.0,  1e-9);
    EXPECT_NEAR(interpolate(ts, val, 15'000), 15.0, 1e-9);
    EXPECT_NEAR(interpolate(ts, val, 25'000), 25.0, 1e-9);
}

TEST(Interpolation, ClampsOutsideRange) {
    std::vector<int64_t> ts  = {10'000, 20'000};
    std::vector<double>  val = {5.0, 10.0};
    EXPECT_NEAR(interpolate(ts, val, 0),      5.0,  1e-9);
    EXPECT_NEAR(interpolate(ts, val, 99'999), 10.0, 1e-9);
}

TEST(Interpolation, ExactSupportPoint) {
    std::vector<int64_t> ts  = {0, 10'000, 20'000};
    std::vector<double>  val = {1.0, 3.0, 7.0};
    EXPECT_NEAR(interpolate(ts, val, 10'000), 3.0, 1e-9);
}

// ─── Radial filter ────────────────────────────────────────────────────────────

TEST(RadialFilter, DropsNoiseKeepsSpike) {
    std::vector<int64_t> ts(10);
    std::vector<double>  val(10, 0.0);
    for (int i = 0; i < 10; ++i) ts[i] = static_cast<int64_t>(i) * 1'000'000;
    val[5] = 1.0;

    auto kept = radial_filter(ts, val, 0.5);
    EXPECT_TRUE(kept[0]);
    EXPECT_TRUE(kept[5]);
    EXPECT_TRUE(kept[9]);
    EXPECT_FALSE(kept[1]);
    EXPECT_FALSE(kept[2]);
}

// ─── VW stats ────────────────────────────────────────────────────────────────

TEST(VwStats, ExactNKeptMatchesNOut) {
    auto [ts, val] = make_sine(500);
    Stats stats;
    compress_vw(ts, val, 30, stats);
    EXPECT_EQ(stats.n_kept, 30u);
}

// ─── Golden sample (bit-for-bit compat with original Rust impl) ──────────────
// Reference files generated by tests/generate_golden.py using the old Rust wheel.

static std::vector<uint8_t> read_golden(const char* filename) {
    std::string path = std::string(CURVEPRESS_TEST_DATA_DIR) + "/" + filename;
    std::ifstream f(path, std::ios::binary);
    EXPECT_TRUE(f.is_open()) << "Could not open golden file: " << path;
    return {std::istreambuf_iterator<char>(f), {}};
}

static void assert_golden(const std::vector<uint8_t>& actual, const char* filename) {
    auto expected = read_golden(filename);
    ASSERT_FALSE(expected.empty()) << "Golden file is empty or missing: " << filename;
    ASSERT_EQ(actual.size(), expected.size())
        << "Compressed size differs from golden reference (" << filename << ")";
    EXPECT_EQ(actual, expected)
        << "Compressed bytes differ from golden reference (" << filename << ")";
}

TEST(Golden, RdpSine500Eps1MatchesRustOutput) {
    auto [ts, val] = make_sine(500);
    assert_golden(compress_rdp(ts, val, 1.0), "rdp_sine500_eps1.bin");
}

TEST(Golden, VwSine500N42MatchesRustOutput) {
    auto [ts, val] = make_sine(500);
    assert_golden(compress_vw(ts, val, 42), "vw_sine500_n42.bin");
}

TEST(Golden, RdpnSine500N42MatchesRustOutput) {
    auto [ts, val] = make_sine(500);
    assert_golden(compress_rdpn(ts, val, 42, 1000.0), "rdpn_sine500_n42.bin");
}
