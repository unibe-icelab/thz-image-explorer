from thz_deconvolution import *
import argparse
import numpy as np
import os
import matplotlib.pyplot as plt

np.set_printoptions(precision=4)

show = False

# Frequency and filter parameters
low_cut = 0.15
high_cut = 10.0
start_freq = 0.2
end_freq = 4.0
win_width = 0.5
n_filters = 20
w_max = 30

print("* Fit parameters: ")
print("  - " + str(start_freq) + " THz to " + str(end_freq) + " THz")
print("  - " + str(n_filters) + " filters")
print("  - " + str(w_max) + " mm max beam width")
print("  - " + str(win_width) + " mm window width")
print()

script_path = os.path.dirname(os.path.abspath(__file__))

# Argument parsing
parser = argparse.ArgumentParser(description="Generate PSF using raw measurements.")
parser.add_argument(
    "--path_x",
    required=True,
    help="Path to the knife edge measurement file in x."
)
parser.add_argument(
    "--path_y",
    required=True,
    help="Path to the knife edge measurement file in y."
)
args = parser.parse_args()

x_path = args.path_x
y_path = args.path_y

# Load the raw PSF measurements
print()
print("* Loading knife edge measurements")

x_psf_lr, y_psf_lr, np_psf_t_x_lr, np_psf_t_y_lr, times_psf = load_knife_edge_meas(x_path, y_path)


x_psf_lr = np.split(x_psf_lr, 2)
y_psf_lr = np.split(y_psf_lr, 2)
np_psf_t_x_lr = np.split(np_psf_t_x_lr, 2)
np_psf_t_y_lr = np.split(np_psf_t_y_lr, 2)

# Flipping the left part
x_psf_lr[0] = -np.flip(x_psf_lr[0])
y_psf_lr[0] = -np.flip(y_psf_lr[0])
np_psf_t_x_lr[0] = np.flip(np_psf_t_x_lr[0])
np_psf_t_y_lr[0] = np.flip(np_psf_t_y_lr[0])

popt_xs_lr = []
popt_ys_lr = []

for x_psf, y_psf, np_psf_t_x, np_psf_t_y in zip(x_psf_lr, y_psf_lr, np_psf_t_x_lr, np_psf_t_y_lr):

    x_psf -= np.mean(x_psf)
    y_psf -= np.mean(y_psf)

    print()
    print("* Fitting the mean PSF")
    n_min = 0
    n_max = -1
    x0, y0, popt_x, popt_y = fit_mean_beam(
        x_psf, y_psf, np_psf_t_x, np_psf_t_y, [n_min, n_max], plot=show)

    # Create the PSF
    x_start = np.abs(x_psf[0])
    y_start = np.abs(y_psf[0])
    dx = np.abs(x_psf[1] - x_psf[0])
    dy = np.abs(y_psf[1] - y_psf[0])
    xx = np.arange(-x_start, x_start + dx, dx)
    yy = np.arange(-y_start, y_start + dy, dy)

    gauss_x = gaussian(xx, 0.0, popt_x[1])
    gauss_y = gaussian(yy, 0.0, popt_y[1])
    gauss_x = gauss_x / np.max(gauss_x)
    gauss_y = gauss_y / np.max(gauss_y)

    _, _, psf_2d = create_psf_2d(gauss_x, gauss_y, xx, yy, plot=False)

    print()
    print("* Creating the filters for the PSF")
    filters, filt_freqs = create_filters(
        n_filters, times_psf, win_width, low_cut, high_cut, start_freq, end_freq, plot=show)

    print()
    print("* Fitting the PSF beam widths by frequency")
    n_min = 0
    n_max = -1
    _, _, popt_xs, popt_ys, _, _ = fit_beam_widths(
        x0, y0, x_psf, y_psf, np_psf_t_x, np_psf_t_y, filters, filt_freqs, w_max, [n_min, n_max], plot=show)

    popt_xs_lr.append(popt_xs)
    popt_ys_lr.append(popt_ys)

popt_xs_lr = np.array(popt_xs_lr)
popt_ys_lr = np.array(popt_ys_lr)

popt_xs_lr[0].T[0] = -popt_xs_lr[0].T[0]
popt_ys_lr[0].T[0] = -popt_ys_lr[0].T[0]

# Averaging
popt_xs = (popt_xs_lr[0] + popt_xs_lr[1]) / 2
popt_ys = (popt_ys_lr[0] + popt_ys_lr[1]) / 2


popt_xs.T[0] -= np.mean(popt_xs.T[0])
popt_ys.T[0] -= np.mean(popt_ys.T[0])


w_xs = popt_xs.T[1]
w_ys = popt_ys.T[1]

x0s = popt_xs.T[0]
y0s = popt_ys.T[0]

# Save the data to a .npz file
data = {
    'low_cut': low_cut,  # float: low cut-off frequency
    'high_cut': high_cut,  # float: high cut-off frequency
    'start_freq': start_freq,  # float: start frequency for filters
    'end_freq': end_freq,  # float: end frequency for filters
    'n_filters': n_filters,  # int: number of filters
    # ndarray: filter coefficients, shape (n_filters, len(times_psf) // 5)
    'filters': filters,
    # ndarray: filter frequencies, shape (n_filters,)
    'filt_freqs': filt_freqs,
    # ndarray: fitted x parameters, shape (n_filters, 2)
    '[x_0, w_x]': popt_xs,
    '[y_0, w_y]': popt_ys  # ndarray: fitted y parameters, shape (n_filters, 2)
}

np.savez(os.path.join(script_path, "sample_directory/psf.npz"), **data)
print()
print("* Data saved to sample_directory/psf.npz")

plt.plot(filt_freqs, w_xs, 'C0')
plt.plot(filt_freqs, w_ys, 'C3')
plt.xlabel("Frequency [THz]")
plt.ylabel("Beam width [mm]")
plt.title("Beam width as a function of frequency")
plt.legend(["Beam width in x", "Beam width in y"])
plt.show()

min_y_range = np.min([np.min(y0s), np.min(x0s)])
max_y_range = np.max([np.max(y0s), np.max(x0s)])

min_y_range = np.min([-5, min_y_range])
max_y_range = np.max([5, max_y_range])

plt.plot(filt_freqs, x0s, 'C0')
plt.plot(filt_freqs, y0s, 'C3')
plt.ylim(min_y_range, max_y_range)
plt.xlabel("Frequency [THz]")
plt.ylabel("Position of the center [mm]")
plt.title("Center of the PSF as a function of frequency")
plt.legend(["Center of the PSF in x", "Center of the PSF in y"])
plt.show()
