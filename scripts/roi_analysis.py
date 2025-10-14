from pathlib import Path
from typing import Optional

from pydotthz import DotthzFile
from shapely.geometry import Polygon, Point


def extract_rois(path: Path, measurement_key: Optional[str] = None):
    """
    Extracts Regions of Interest (ROIs) from a DotthzFile and returns the pixel coordinates inside each ROI.

    Parameters:
    path (Path): Path to the DotthzFile containing the image data.
    measurement_key (Optional[str]): The key for selecting a specific measurement. Defaults to the first measurement.

    Returns:
    dict: A dictionary mapping ROI labels to lists of pixel coordinates within each ROI.
    """
    rois = {}
    with DotthzFile(path, "r") as image_file:
        # Read the measurement
        if measurement_key is None:
            # Just get the first one
            measurement_key = list(image_file.get_measurements().keys())[0]

        _datasets = image_file[measurement_key].datasets
        metadata = image_file[measurement_key].metadata

        # Get image dimensions
        height, width = int(float(metadata["height"])), int(float(metadata["width"]))

        # Extract and parse ROI polygon from metadata
        for (index, roi_label) in enumerate(metadata["ROI Labels"].split(",")):
            roi_raw = metadata[f"ROI {index}"]  # Assuming ROI is stored as a string like "[[x1,y1],[x2,y2],...]"
            roi_points = eval(roi_raw) if isinstance(roi_raw, str) else roi_raw  # Convert to list of lists
            roi_points = list(roi_points)  # Ensure it's a list

            # Convert to image coordinate system (flip Y and swap x and y)
            roi_points_corrected = [(width - 1 - y, x) for x, y in roi_points]
            polygon = Polygon(roi_points_corrected)

            # Find all pixels inside the ROI
            pixels_inside_roi = [
                (x, y) for y in range(height) for x in range(width) if polygon.contains(Point(x, y))
            ]
            rois[roi_label] = pixels_inside_roi

    if len(rois) == 0:
        raise Exception(f"No ROIs found in {path}")

    return rois  # Returns list of pixel coordinates
