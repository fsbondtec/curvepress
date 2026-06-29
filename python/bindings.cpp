/// pybind11 bindings for curvepress.
///pip install -i https://test.pypi.org/simple/ curvepress
/// numpy arrays (int64 timestamps, float64 values) are accepted as input.
/// compress_* functions return bytes; decompress returns a tuple (ts, values).
#include <pybind11/numpy.h>
#include <pybind11/pybind11.h>
#include <pybind11/stl.h>

#include <curvepress/curvepress.hpp>

namespace py = pybind11;
using namespace curvepress;

// ── helpers ───────────────────────────────────────────────────────────────────

static std::span<const int64_t> ts_span(const py::array_t<int64_t>& arr) {
    auto buf = arr.request();
    return { static_cast<const int64_t*>(buf.ptr), static_cast<std::size_t>(buf.size) };
}

static std::span<const double> val_span(const py::array_t<double>& arr) {
    auto buf = arr.request();
    return { static_cast<const double*>(buf.ptr), static_cast<std::size_t>(buf.size) };
}

static py::bytes to_bytes(const std::vector<uint8_t>& v) {
    return py::bytes(reinterpret_cast<const char*>(v.data()), v.size());
}

static py::dict stats_to_dict(const Stats& s) {
    py::dict d;
    d["n_input"]          = s.n_input;
    d["n_kept"]           = s.n_kept;
    d["bytes_raw"]        = s.bytes_raw;
    d["bytes_compressed"] = s.bytes_compressed;
    d["ratio"]            = s.ratio;
    d["max_error"]        = s.max_error;
    d["quant_bits"]       = s.quant_bits;
    return d;
}

// ── module ────────────────────────────────────────────────────────────────────

PYBIND11_MODULE(_curvepress, m) {
    m.doc() = "curvepress – lossy time-series compression (C++ core, pybind11 bindings)";

    // compress_rdp
    m.def("compress_rdp",
        [](py::array_t<int64_t> ts, py::array_t<double> val, double epsilon) {
            return to_bytes(compress_rdp(ts_span(ts), val_span(val), epsilon));
        },
        py::arg("timestamps"), py::arg("values"), py::arg("epsilon"),
        "Compress with RDP. Returns compressed bytes.");

    m.def("compress_rdp_stats",
        [](py::array_t<int64_t> ts, py::array_t<double> val, double epsilon) {
            Stats s;
            auto data = compress_rdp(ts_span(ts), val_span(val), epsilon, s);
            return py::make_tuple(to_bytes(data), stats_to_dict(s));
        },
        py::arg("timestamps"), py::arg("values"), py::arg("epsilon"),
        "Compress with RDP. Returns (bytes, stats_dict).");

    // compress_vw
    m.def("compress_vw",
        [](py::array_t<int64_t> ts, py::array_t<double> val, std::size_t n_out) {
            return to_bytes(compress_vw(ts_span(ts), val_span(val), n_out));
        },
        py::arg("timestamps"), py::arg("values"), py::arg("n_out"),
        "Compress with Visvalingam-Whyatt. Returns compressed bytes.");

    m.def("compress_vw_stats",
        [](py::array_t<int64_t> ts, py::array_t<double> val, std::size_t n_out) {
            Stats s;
            auto data = compress_vw(ts_span(ts), val_span(val), n_out, s);
            return py::make_tuple(to_bytes(data), stats_to_dict(s));
        },
        py::arg("timestamps"), py::arg("values"), py::arg("n_out"),
        "Compress with VW. Returns (bytes, stats_dict).");

    // compress_rdpn
    m.def("compress_rdpn",
        [](py::array_t<int64_t> ts, py::array_t<double> val, std::size_t n_out, double epsilon) {
            return to_bytes(compress_rdpn(ts_span(ts), val_span(val), n_out, epsilon));
        },
        py::arg("timestamps"), py::arg("values"), py::arg("n_out"), py::arg("epsilon"),
        "Compress with RDP-N (binary-searched epsilon). Returns compressed bytes.");

    m.def("compress_rdpn_stats",
        [](py::array_t<int64_t> ts, py::array_t<double> val, std::size_t n_out, double epsilon) {
            Stats s;
            auto data = compress_rdpn(ts_span(ts), val_span(val), n_out, epsilon, s);
            return py::make_tuple(to_bytes(data), stats_to_dict(s));
        },
        py::arg("timestamps"), py::arg("values"), py::arg("n_out"), py::arg("epsilon"),
        "Compress with RDP-N. Returns (bytes, stats_dict).");

    // decompress
    m.def("decompress",
        [](py::bytes data) {
            py::buffer_info info = py::buffer(data).request();
            std::span<const uint8_t> span{
                reinterpret_cast<const uint8_t*>(info.ptr),
                static_cast<std::size_t>(info.size)
            };
            auto dec = decompress(span);
            std::size_t n = dec.timestamps_ns.size();

            py::array_t<int64_t> ts_arr(static_cast<py::ssize_t>(n));
            py::array_t<double>  val_arr(static_cast<py::ssize_t>(n));
            std::copy(dec.timestamps_ns.begin(), dec.timestamps_ns.end(),
                      ts_arr.mutable_data());
            std::copy(dec.values.begin(), dec.values.end(),
                      val_arr.mutable_data());
            return py::make_tuple(ts_arr, val_arr);
        },
        py::arg("data"),
        "Decompress bytes into (timestamps_ns, values) numpy arrays.");

    // interpolate
    m.def("interpolate",
        [](py::array_t<int64_t> ts, py::array_t<double> val, int64_t t) {
            return interpolate(ts_span(ts), val_span(val), t);
        },
        py::arg("timestamps"), py::arg("values"), py::arg("t"),
        "Linear interpolation at timestamp t. Clamps outside data range.");

    // version
    m.def("version", &curvepress::version, "Library version string.");
}
