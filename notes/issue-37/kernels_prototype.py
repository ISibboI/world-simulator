"""Issue #37 feasibility check (2026-11-27): surface dynamic-topography kernels of a layered
viscous sphere vs the model's Gaussian slab stages.

Propagator method of Hager & O'Connell (1981) / Hager & Clayton (1989): poloidal state vector
y = (u_r, u_theta, r sigma_rr / eta0, r sigma_rtheta / eta0), dy/dln r = A(l, eta) y; the matrix
was verified symbolically against the analytic constant-viscosity Stokes solutions (r^n,
n = l+1, l-1, -l, -l-2). Free slip at surface and CMB. Kernel K_l(d) = fraction of the weight of
a degree-l load at depth d supported by surface deflection (1 at the surface, 0 at the CMB).
Checked against the half-space limit (1 + kd) exp(-kd) (l = 40, d = 330 km: 0.376 vs 0.387).

Usage: python3 kernels_prototype.py   (runs ~1-2 min)
"""
import math
import numpy as np

R, C = 6371e3, 3480e3
RHO_M = 3300.0
SECONDS_PER_MYR = 1e6 * 365.25 * 24 * 3600
LMAX = 50


def a_matrix(l, eta):
    L = l * (l + 1)
    return np.array([[-2, L, 0, 0], [-1, 1, 0, 1 / eta],
                     [12 * eta, -6 * L * eta, 1, L], [-6 * eta, 2 * (2 * L - 1) * eta, -1, -2]], float)


def viscosity(depth, profile):
    for _, bottom, eta in profile:
        if depth < bottom:
            return eta
    return profile[-1][2]


def propagate(y, l, x0, x1, profile, steps):
    h = (x1 - x0) / steps
    f = lambda x, v: a_matrix(l, viscosity(R - math.exp(x) * R, profile)) @ v
    for i in range(steps):
        x = x0 + i * h
        k1 = f(x, y); k2 = f(x + h / 2, y + h / 2 * k1); k3 = f(x + h / 2, y + h / 2 * k2); k4 = f(x + h, y + h * k3)
        y = y + h / 6 * (k1 + 2 * k2 + 2 * k3 + k4)
    return y


def kernel(l, depth, profile, steps_per=150):
    rp = (R - depth) / R
    def integrate(Y, x0, x1):
        cuts = [math.log((R - b) / R) for _, b, _ in profile if R - b > 0]
        bounds = sorted({x0, x1, *[c for c in cuts if min(x0, x1) < c < max(x0, x1)]})
        if x1 < x0:
            bounds = bounds[::-1]
        for a, b in zip(bounds[:-1], bounds[1:]):
            n = max(4, int(abs(b - a) * steps_per * (l + 3) / 10))
            Y = np.column_stack([propagate(Y[:, k], l, a, b, profile, n) for k in range(Y.shape[1])])
        return Y
    basis = np.zeros((4, 2)); basis[1, 0] = 1; basis[2, 1] = 1  # u_r = 0, sigma_rtheta = 0
    below = integrate(basis.copy(), math.log(C / R), math.log(rp))
    above = integrate(basis.copy(), 0.0, math.log(rp))
    jump = np.array([0, 0, rp, 0.0])  # jump of r sigma_rr / eta0 for m g / eta0 = 1
    solution = np.linalg.solve(np.column_stack([above, -below]), jump)
    return solution[1] / rp ** 2


def stage_kernel(d0, d1, profile, n=4):
    depths = [d0 + (d1 - d0) * (i + 0.5) / n for i in range(n)]
    return np.array([0.0] + [np.mean([kernel(l, d, profile) for d in depths]) for l in range(1, LMAX + 1)])


def green(K, cos, taper):
    l = np.arange(LMAX + 1)
    P = np.zeros((LMAX + 1,) + cos.shape); P[0] = 1; P[1] = cos
    for k in range(1, LMAX):
        P[k + 1] = ((2 * k + 1) * cos * P[k] - k * P[k - 1]) / (k + 1)
    weights = (2 * l + 1) * K * np.exp(-l * (l + 1) * (taper / R) ** 2 / 2)
    return -np.tensordot(weights, P, axes=1) / (4 * math.pi * RHO_M * R * R)


def gaussian(f, sigma, cos):
    return -f / (RHO_M * 2 * math.pi * sigma ** 2) * np.exp(-2 * (1 - cos) / (2 * (sigma / R) ** 2))


# Synthetic case of the unit test: 5000 km trench, 5 cm/yr, old plate (7e6 kg/m2), steady state.
flux = 5e6 * 0.05 / (365.25 * 24 * 3600) * 7e6
loads = np.array([(math.cos(i * 100e3 / R), 0, math.sin(i * 100e3 / R)) for i in range(-25, 26)])
distances = np.array([0, 600, 1500]) * 1e3
cos = np.array([(math.cos(d / R), math.sin(d / R), 0) for d in distances]) @ loads.T
# (residence Myr, current response, current width, depth range, alpha relative to the surface)
stages = [(13, 0.5, 600e3, (100e3, 660e3), 1.0), (45, 0.4, 900e3, (660e3, 1200e3), 0.65),
          (67, 0.2, 1500e3, (1200e3, 2000e3), 0.45)]
old = sum(r * flux * SECONDS_PER_MYR / len(loads) * gaussian(f, s, cos).sum(axis=1) for r, f, s, _, _ in stages)
print("current Gaussian stages, h at 0/600/1500 km from the trench line (m):", np.round(old))
profiles = {
    "lith 100x / UM 1 / LM 30x": [(0, 100e3, 100.0), (100e3, 660e3, 1.0), (660e3, 1e9, 30.0)],
    "lith 100x / asth 0.1x (100-300) / TZ 1 / LM 30x": [(0, 100e3, 100.0), (100e3, 300e3, 0.1), (300e3, 660e3, 1.0), (660e3, 1e9, 30.0)],
    "lith 100x / UM 1 / LM 100x": [(0, 100e3, 100.0), (100e3, 660e3, 1.0), (660e3, 1e9, 100.0)],
}
for name, profile in profiles.items():
    total, with_alpha, ks = np.zeros(3), np.zeros(3), []
    for r, _, _, (d0, d1), alpha in stages:
        K = stage_kernel(d0, d1, profile)
        ks.append(np.round(K[[2, 5, 10, 20]], 2).tolist())
        h = r * flux * SECONDS_PER_MYR / len(loads) * green(K, cos, 300e3).sum(axis=1)
        total += h; with_alpha += alpha * h
    print(name, "\n  K(l=2,5,10,20) per stage:", ks, "\n  h (m):", np.round(total), " with alpha(P):", np.round(with_alpha))
