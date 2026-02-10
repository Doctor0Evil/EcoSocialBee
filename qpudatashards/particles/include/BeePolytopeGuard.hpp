#pragma once
#include <array>
#include <string>
#include <vector>
#include <cstdint>
#include <cmath>

namespace EcoNet {

struct State5D {
    double pm25_ug_m3;   // PM2.5 [µg/m³]
    double nox_ppb;      // NOx as NO2 [ppb]
    double o3_ppb;       // O3 [ppb]
    double temp_C;       // Air temperature [°C]
    double floral_m2_ha; // Floral density [m²/ha]
};

struct BeeHazardParams {
    // LC50 values from Phoenix/Region 9 mixture studies
    double lc50_pm25_ug_m3;
    double lc50_nox_ppb;
    double lc50_o3_ppb;
    double lc50_voc_ug_m3;

    // Mixture weights (sum to 1.0)
    double w_pm25;
    double w_nox;
    double w_o3;
    double w_voc;

    // Preferred PM2.5-equivalent viability corridor
    double pm25_eq_viability_ug_m3; // e.g., 8.5 µg/m³
};

struct BeeHazardState {
    double r_bee;        // Instantaneous bee risk index (0–∞)
    double pm25_eq;      // PM2.5-equivalent index [µg/m³]
    bool   within_viability;
};

struct Polytope {
    // Represents {x in R^5 | A x <= b}
    // A: m x 5, b: m
    std::vector<std::array<double, 5>> A;
    std::vector<double>                b;
};

enum class RegionClass : std::uint8_t {
    FORAGE_SAFE = 0,
    RETREAT_ONLY = 1,
    FORBIDDEN = 2
};

struct BeeActuationLimits {
    RegionClass region;
    double duty_scale;     // 0–1 multiplier for nanoswarm actuators
    bool   allow_foraging; // whether bees should be encouraged to forage
};

class BeePolytopeGuard {
public:
    BeePolytopeGuard(const BeeHazardParams& hz,
                     const Polytope& foragePoly,
                     const Polytope& retreatPoly,
                     double rBeeSoftLimit,
                     double rBeeHardLimit)
        : params_(hz),
          foragePoly_(foragePoly),
          retreatPoly_(retreatPoly),
          rBeeSoftLimit_(rBeeSoftLimit),
          rBeeHardLimit_(rBeeHardLimit)
    {}

    BeeHazardState computeHazard(const State5D& s,
                                 double voc_ug_m3) const {
        BeeHazardState h{};
        const double r_pm =
            (params_.lc50_pm25_ug_m3 > 0.0)
                ? s.pm25_ug_m3 / params_.lc50_pm25_ug_m3 : 0.0;
        const double r_nox =
            (params_.lc50_nox_ppb > 0.0)
                ? s.nox_ppb / params_.lc50_nox_ppb : 0.0;
        const double r_o3 =
            (params_.lc50_o3_ppb > 0.0)
                ? s.o3_ppb / params_.lc50_o3_ppb : 0.0;
        const double r_voc =
            (params_.lc50_voc_ug_m3 > 0.0)
                ? voc_ug_m3 / params_.lc50_voc_ug_m3 : 0.0;

        h.r_bee =
            params_.w_pm25 * r_pm +
            params_.w_nox  * r_nox +
            params_.w_o3   * r_o3 +
            params_.w_voc  * r_voc;

        // Simple PM2.5-equivalent using mixture weights
        h.pm25_eq =
            s.pm25_ug_m3 +
            params_.w_nox * s.nox_ppb +
            params_.w_o3  * s.o3_ppb +
            params_.w_voc * voc_ug_m3;

        h.within_viability =
            (h.pm25_eq <= params_.pm25_eq_viability_ug_m3) &&
            (h.r_bee <= rBeeSoftLimit_);
        return h;
    }

    RegionClass classifyRegion(const State5D& s) const {
        const bool inForage = inPolytope(foragePoly_, s);
        if (inForage) {
            return RegionClass::FORAGE_SAFE;
        }
        const bool inRetreat = inPolytope(retreatPoly_, s);
        if (inRetreat) {
            return RegionClass::RETREAT_ONLY;
        }
        return RegionClass::FORBIDDEN;
    }

    BeeActuationLimits computeActuation(const State5D& s,
                                        double voc_ug_m3) const {
        const BeeHazardState hz = computeHazard(s, voc_ug_m3);
        const RegionClass region = classifyRegion(s);

        BeeActuationLimits lim{};
        lim.region = region;

        if (region == RegionClass::FORBIDDEN ||
            hz.r_bee >= rBeeHardLimit_) {
            lim.duty_scale = 0.0;
            lim.allow_foraging = false;
            return lim;
        }

        if (region == RegionClass::RETREAT_ONLY ||
            hz.r_bee > rBeeSoftLimit_) {
            // Scale down duty in proportion to risk (linear clip)
            const double alpha =
                std::max(0.0, 1.0 - hz.r_bee / rBeeHardLimit_);
            lim.duty_scale = alpha * 0.5;
            lim.allow_foraging = false;
            return lim;
        }

        // Forage-safe region with low bee risk
        lim.duty_scale = 1.0;
        lim.allow_foraging = true;
        return lim;
    }

private:
    static bool inPolytope(const Polytope& P, const State5D& s) {
        const std::array<double,5> x{
            s.pm25_ug_m3,
            s.nox_ppb,
            s.o3_ppb,
            s.temp_C,
            s.floral_m2_ha
        };
        const std::size_t m = P.A.size();
        for (std::size_t i = 0; i < m; ++i) {
            const auto& row = P.A[i];
            double dot = 0.0;
            for (std::size_t j = 0; j < 5; ++j) {
                dot += row[j] * x[j];
            }
            if (dot > P.b[i] + 1e-9) {
                return false;
            }
        }
        return true;
    }

    BeeHazardParams params_;
    Polytope        foragePoly_;
    Polytope        retreatPoly_;
    double          rBeeSoftLimit_;
    double          rBeeHardLimit_;
};

} // namespace EcoNet
