#include <catch2/catch_test_macros.hpp>
#include <catch2/matchers/catch_matchers_floating_point.hpp>
#include <curvepress/curvepress.hpp>
#include <cmath>
#include <vector>

using namespace curvepress;
using Catch::Matchers::WithinAbs;

// ─── helpers ─────────────────────────────────────────────────────────────────

static std::pair<std::vector<int64_t>, std::vector<double>>
make_sine(std::size_t n) {
    std::vector<int64_t> ts(n);
    std::vector<double>  val(n);
    for (std::size_t i = 0; i < n; ++i) {
        ts[i]  = static_cast<int64_t>(i) * 1'000'000;
        val[i] = std::sin(static_cast<double>(i) * 0.05) * 100.0;
    }
    return {ts, val};
}

static std::pair<std::vector<int64_t>, std::vector<double>>
make_fracture(std::size_t n = 500) {
    std::vector<int64_t> ts(n);
    std::vector<double>  val(n);
    for (std::size_t i = 0; i < n; ++i) ts[i] = static_cast<int64_t>(i) * 1'000'000;
    const std::size_t ramp_end = 300, peak = 300, drop_end = 310;
    for (std::size_t i = 0; i < ramp_end; ++i)
        val[i] = static_cast<double>(i) / 3.0;
    if (peak < n) val[peak] = 150.0;
    for (std::size_t i = peak + 1; i < std::min(drop_end, n); ++i)
        val[i] = std::max(0.0, 100.0 - static_cast<double>(i - peak) * 11.0);
    for (std::size_t i = drop_end; i < n; ++i)
        val[i] = 0.5 * std::abs(std::sin(static_cast<double>(i) * 0.3));
    return {ts, val};
}

// ─── Tests ───────────────────────────────────────────────────────────────────

TEST_CASE("RDP round-trip reduces point count", "[rdp]") {
    auto [ts, val] = make_sine(1000);
    auto data = compress_rdp(ts, val, 1.0);
    auto dec  = decompress(data);
    CHECK(dec.timestamps_ns.size() < ts.size());
    CHECK(dec.timestamps_ns.size() == dec.values.size());
}

TEST_CASE("VW round-trip returns exactly n_out points", "[vw]") {
    auto [ts, val] = make_sine(500);
    auto data = compress_vw(ts, val, 40);
    auto dec  = decompress(data);
    CHECK(dec.timestamps_ns.size() == 40);
}

TEST_CASE("Fracture curve: peak and first post-drop point kept", "[fracture]") {
    auto [ts, val] = make_fracture();
    auto data = compress_rdp(ts, val, 1.0);
    auto dec  = decompress(data);

    int64_t peak_ts      = ts[300];
    int64_t post_drop_ts = ts[301];
    bool peak_found = false, post_found = false;
    for (auto t : dec.timestamps_ns) {
        if (t == peak_ts)      peak_found = true;
        if (t == post_drop_ts) post_found = true;
    }
    CHECK(peak_found);
    CHECK(post_found);
}

TEST_CASE("Exception on bad input", "[errors]") {
    std::vector<int64_t> ts = {0, 2'000'000, 1'000'000}; // non-monotonic
    std::vector<double>  val = {0.0, 1.0, 2.0};
    CHECK_THROWS_AS(compress_rdp(ts, val, 1.0), std::invalid_argument);
}

TEST_CASE("Dry-run sizing via C ABI", "[capi]") {
    auto [ts, val] = make_sine(200);
    std::size_t out_len = 0;
    int rc = cp_compress_rdp(ts.data(), val.data(), ts.size(),
                             1.0, nullptr, 0, &out_len, nullptr);
    CHECK(rc == 0);
    CHECK(out_len > 0);
    CHECK(out_len < ts.size() * 16);
}

TEST_CASE("Interpolate midpoint", "[interpolate]") {
    std::vector<int64_t> ts  = {0, 10'000, 20'000, 30'000};
    std::vector<double>  val = {0.0, 10.0, 20.0, 30.0};
    CHECK_THAT(interpolate(ts, val, 5'000),  WithinAbs(5.0,  1e-9));
    CHECK_THAT(interpolate(ts, val, 15'000), WithinAbs(15.0, 1e-9));
    CHECK_THAT(interpolate(ts, val, 25'000), WithinAbs(25.0, 1e-9));
}

TEST_CASE("Interpolate clamps outside range", "[interpolate]") {
    std::vector<int64_t> ts  = {0, 10'000};
    std::vector<double>  val = {3.0, 7.0};
    CHECK_THAT(interpolate(ts, val, -5'000), WithinAbs(3.0, 1e-9));
    CHECK_THAT(interpolate(ts, val, 20'000), WithinAbs(7.0, 1e-9));
}

TEST_CASE("RDP-N returns at most n_out points", "[rdpn]") {
    auto [ts, val] = make_sine(1000);
    auto data = compress_rdpn(ts, val, 50, 100.0);
    auto dec  = decompress(data);
    CHECK(dec.timestamps_ns.size() <= 50);
}

TEST_CASE("Stats: max_error is finite", "[stats]") {
    auto [ts, val] = make_sine(2000);
    Stats stats;
    compress_rdp(ts, val, 2.0, &stats);
    CHECK(std::isfinite(stats.max_error));
    CHECK(stats.max_error >= 0.0);
}
