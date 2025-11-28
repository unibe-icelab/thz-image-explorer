#!/usr/bin/env python3
"""
Script to test loading PSF fit coefficients exported in NPZ format.
Supports hybrid model format: base model (a/f + b) + correction spline for wx/wy.
"""
import numpy as np
import sys


def eval_spline(f, knots, a, b, c, d):
    """
    Evaluate cubic spline at frequency f.
    For segment i where knots[i] <= f < knots[i+1]:
    value(f) = a[i] + b[i]*(f - knots[i]) + c[i]*(f - knots[i])^2 + d[i]*(f - knots[i])^3
    """
    i = np.searchsorted(knots[:-1], f, side='right') - 1
    i = max(0, min(i, len(a) - 1))
    dx = f - knots[i]
    return a[i] + b[i]*dx + c[i]*dx**2 + d[i]*dx**3


def eval_hybrid_fit(f, base_a, base_b, corr_knots, corr_a, corr_b, corr_c, corr_d):
    """
    Evaluate hybrid model: base + correction spline.
    base(f) = a/f + b
    """
    base = base_a / f + base_b
    correction = eval_spline(f, corr_knots, corr_a, corr_b, corr_c, corr_d)
    return base + correction


def test_npz_file(filename):
    """Test loading and displaying contents of NPZ file"""
    print(f"Loading {filename}...")

    try:
        data = np.load(filename)

        print("\n=== Contents of NPZ file ===")
        print(f"Number of arrays: {len(data.files)}")
        print(f"Array names: {data.files}")

        print("\n=== Array details ===")
        for name in sorted(data.files):
            arr = data[name]
            print(f"\n{name}:")
            print(f"  Shape: {arr.shape}")
            print(f"  Dtype: {arr.dtype}")
            print(f"  Min: {arr.min():.6f}")
            print(f"  Max: {arr.max():.6f}")
            if len(arr) <= 10:
                print(f"  Values: {arr}")
            else:
                print(f"  First 3: {arr[:3]}")
                print(f"  Last 3: {arr[-3:]}")

        # Verify expected arrays
        # Detect format: check if we have hybrid model or old pure spline format
        has_hybrid = "wx_base_a" in data.files
        
        if has_hybrid:
            print("\n📊 Format: HYBRID MODEL (base + correction splines)")
            # For wx/wy: hybrid model (base + correction spline)
            hybrid_fits = ["wx", "wy"]
            hybrid_components = [
                "base_a",
                "base_b",
                "corr_knots_thz",
                "corr_values_mm",
                "corr_coeff_a",
                "corr_coeff_b",
                "corr_coeff_c",
                "corr_coeff_d",
            ]

            # For x0/y0: pure splines
            spline_fits = ["x0", "y0"]
            spline_components = [
                "knots_thz",
                "values_mm",
                "coeff_a",
                "coeff_b",
                "coeff_c",
                "coeff_d",
            ]

            expected = []
            for fit in hybrid_fits:
                for comp in hybrid_components:
                    expected.append(f"{fit}_{comp}")
            for fit in spline_fits:
                for comp in spline_components:
                    expected.append(f"{fit}_{comp}")

            missing = [name for name in expected if name not in data.files]
            if missing:
                print(f"\n⚠️  Warning: Missing expected arrays: {missing}")
            else:
                print(f"\n✓ All expected arrays present (hybrid + spline models)")
        else:
            print("\n📊 Format: LEGACY (pure splines for all parameters)")
            # Old format: all parameters use pure splines
            fits = ["wx", "wy", "x0", "y0"]
            components = [
                "knots_thz",
                "values_mm",
                "coeff_a",
                "coeff_b",
                "coeff_c",
                "coeff_d",
            ]
            
            expected = []
            for fit in fits:
                for comp in components:
                    expected.append(f"{fit}_{comp}")
            
            missing = [name for name in expected if name not in data.files]
            if missing:
                print(f"\n⚠️  Warning: Missing expected arrays: {missing}")
            else:
                print(f"\n✓ All expected spline coefficient arrays present")
        
        if has_hybrid:
            # Check hybrid model structure (wx, wy)
            print(f"\n=== Hybrid model structure (wx, wy) ===")
            for fit in hybrid_fits:
                base_a_key = f"{fit}_base_a"
                base_b_key = f"{fit}_base_b"
                knots_key = f"{fit}_corr_knots_thz"
                values_key = f"{fit}_corr_values_mm"

                if all(k in data.files for k in [base_a_key, base_b_key, knots_key, values_key]):
                    base_a = data[base_a_key][0]
                    base_b = data[base_b_key][0]
                    knots = data[knots_key]
                    values = data[values_key]
                    n_knots = len(knots)
                    n_values = len(values)

                    print(f"\n{fit.upper()} hybrid fit:")
                    print(f"  Base model: w = {base_a:.6f}/f + {base_b:.6f}")
                    print(f"  Correction spline: {n_knots} knots")
                    print(f"  Frequency range: {knots.min():.3f} - {knots.max():.3f} THz")

                    # Check coefficients
                    for coeff in ["a", "b", "c", "d"]:
                        coeff_key = f"{fit}_corr_coeff_{coeff}"
                        if coeff_key in data.files:
                            coeff_arr = data[coeff_key]
                            n_segments = len(coeff_arr)
                            if coeff == "a":
                                print(f"  Correction coefficients: {n_segments} segments")

                            if n_segments != n_knots - 1:
                                print(
                                    f"    ⚠️  Expected {n_knots - 1} segments for {n_knots} knots"
                                )

                    if n_knots == n_values:
                        print(f"  ✓ Knots and values match")
                    else:
                        print(f"  ⚠️  Knots ({n_knots}) and values ({n_values}) don't match")

            # Check pure spline structure (x0, y0)
            print(f"\n=== Pure spline structure (x0, y0) ===")
            for fit in spline_fits:
                knots_key = f"{fit}_knots_thz"
                values_key = f"{fit}_values_mm"

                if knots_key in data.files and values_key in data.files:
                    knots = data[knots_key]
                    values = data[values_key]
                    n_knots = len(knots)
                    n_values = len(values)

                    print(f"\n{fit.upper()} spline:")
                    print(f"  Number of knots: {n_knots}")
                    print(f"  Number of values: {n_values}")
                    print(f"  Frequency range: {knots.min():.3f} - {knots.max():.3f} THz")

                    # Check coefficients
                    for coeff in ["a", "b", "c", "d"]:
                        coeff_key = f"{fit}_coeff_{coeff}"
                        if coeff_key in data.files:
                            coeff_arr = data[coeff_key]
                            n_segments = len(coeff_arr)
                            if coeff == "a":
                                print(f"  Coefficients: {n_segments} segments")

                            if n_segments != n_knots - 1:
                                print(
                                    f"    ⚠️  Expected {n_knots - 1} segments for {n_knots} knots"
                                )

                    if n_knots == n_values:
                        print(f"  ✓ Knots and values match")
                    else:
                        print(f"  ⚠️  Knots ({n_knots}) and values ({n_values}) don't match")
        else:
            # Legacy format: check all as pure splines
            print(f"\n=== Spline structure consistency checks ===")
            for fit in fits:
                knots_key = f"{fit}_knots_thz"
                values_key = f"{fit}_values_mm"

                if knots_key in data.files and values_key in data.files:
                    knots = data[knots_key]
                    values = data[values_key]
                    n_knots = len(knots)
                    n_values = len(values)

                    print(f"\n{fit.upper()} spline:")
                    print(f"  Number of knots: {n_knots}")
                    print(f"  Number of values: {n_values}")
                    print(f"  Frequency range: {knots.min():.3f} - {knots.max():.3f} THz")

                    # Check coefficients
                    for coeff in ["a", "b", "c", "d"]:
                        coeff_key = f"{fit}_coeff_{coeff}"
                        if coeff_key in data.files:
                            coeff_arr = data[coeff_key]
                            n_segments = len(coeff_arr)
                            print(f"  Coefficient {coeff}: {n_segments} segments")

                            if n_segments != n_knots - 1:
                                print(
                                    f"    ⚠️  Expected {n_knots - 1} segments for {n_knots} knots"
                                )

                    if n_knots == n_values:
                        print(f"  ✓ Knots and values match")
                    else:
                        print(f"  ⚠️  Knots ({n_knots}) and values ({n_values}) don't match")
            knots_key = f"{fit}_knots_thz"
            values_key = f"{fit}_values_mm"

            if knots_key in data.files and values_key in data.files:
                knots = data[knots_key]
                values = data[values_key]
                n_knots = len(knots)
                n_values = len(values)

                print(f"\n{fit.upper()} spline:")
                print(f"  Number of knots: {n_knots}")
                print(f"  Number of values: {n_values}")
                print(f"  Frequency range: {knots.min():.3f} - {knots.max():.3f} THz")

                # Check coefficients
                for coeff in ["a", "b", "c", "d"]:
                    coeff_key = f"{fit}_coeff_{coeff}"
                    if coeff_key in data.files:
                        coeff_arr = data[coeff_key]
                        n_segments = len(coeff_arr)
                        print(f"  Coefficient {coeff}: {n_segments} segments")

                        if n_segments != n_knots - 1:
                            print(
                                f"    ⚠️  Expected {n_knots - 1} segments for {n_knots} knots"
                            )

                if n_knots == n_values:
                    print(f"  ✓ Knots and values match")
                else:
                    print(f"  ⚠️  Knots ({n_knots}) and values ({n_values}) don't match")

        # Test model evaluation
        print(f"\n=== Testing model reconstruction ===")
        
        if has_hybrid:
            # Test hybrid fits (wx, wy)
            for fit in hybrid_fits:
                base_a_key = f"{fit}_base_a"
                base_b_key = f"{fit}_base_b"
                knots_key = f"{fit}_corr_knots_thz"
                
                if all(k in data.files for k in [base_a_key, base_b_key, knots_key]):
                    base_a = data[base_a_key][0]
                    base_b = data[base_b_key][0]
                    knots = data[knots_key]
                    corr_a = data[f"{fit}_corr_coeff_a"]
                    corr_b = data[f"{fit}_corr_coeff_b"]
                    corr_c = data[f"{fit}_corr_coeff_c"]
                    corr_d = data[f"{fit}_corr_coeff_d"]
                    
                    # Test at a few points
                    test_freqs = np.linspace(knots.min(), knots.max(), 5)
                    
                    print(f"\n{fit.upper()} (hybrid model) at test frequencies:")
                    for f in test_freqs:
                        value = eval_hybrid_fit(f, base_a, base_b, knots, corr_a, corr_b, corr_c, corr_d)
                        base_only = base_a / f + base_b
                        correction = value - base_only
                        print(f"  f={f:.2f} THz: {value:.6f} mm (base: {base_only:.6f}, correction: {correction:+.6f})")
            
            # Test pure splines (x0, y0)
            for fit in spline_fits:
                knots_key = f"{fit}_knots_thz"
                
                if knots_key in data.files:
                    knots = data[knots_key]
                    coeff_a = data[f"{fit}_coeff_a"]
                    coeff_b = data[f"{fit}_coeff_b"]
                    coeff_c = data[f"{fit}_coeff_c"]
                    coeff_d = data[f"{fit}_coeff_d"]
                    
                    # Test at a few points
                    test_freqs = np.linspace(knots.min(), knots.max(), 5)
                    
                    print(f"\n{fit.upper()} (pure spline) at test frequencies:")
                    for f in test_freqs:
                        value = eval_spline(f, knots, coeff_a, coeff_b, coeff_c, coeff_d)
                        print(f"  f={f:.2f} THz: {value:.6f} mm")
        else:
            # Legacy format: all parameters as pure splines
            for fit in fits:
                knots_key = f"{fit}_knots_thz"
                
                if knots_key in data.files:
                    knots = data[knots_key]
                    coeff_a = data[f"{fit}_coeff_a"]
                    coeff_b = data[f"{fit}_coeff_b"]
                    coeff_c = data[f"{fit}_coeff_c"]
                    coeff_d = data[f"{fit}_coeff_d"]
                    
                    # Test at a few points
                    test_freqs = np.linspace(knots.min(), knots.max(), 5)
                    
                    print(f"\n{fit.upper()} (pure spline) at test frequencies:")
                    for f in test_freqs:
                        value = eval_spline(f, knots, coeff_a, coeff_b, coeff_c, coeff_d)
                        print(f"  f={f:.2f} THz: {value:.6f} mm")
                    value = eval_spline(f, knots, coeff_a, coeff_b, coeff_c, coeff_d)
                    print(f"  f={f:.2f} THz: {value:.6f} mm")

        data.close()
        print("\n✓ NPZ file loaded successfully!")
        return True

    except Exception as e:
        print(f"\n✗ Error loading NPZ file: {e}")
        import traceback

        traceback.print_exc()
        return False


if __name__ == "__main__":
    if len(sys.argv) > 1:
        filename = sys.argv[1]
    else:
        filename = "psf_coefficients.npz"

    success = test_npz_file(filename)
    sys.exit(0 if success else 1)
