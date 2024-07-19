/// JS implementation of accurate Mercator projection,
/// to be used at larger scales
/// Copyright 2006 Christopher Schmidt
/// see: http://wiki.openstreetmap.org/wiki/Mercator
///


var BREAKING_POINT_METER = 150000;
var ROUNDING_FACTOR = 100000000;

var sm_a = 6378137.0;
var sm_b = 6356752.314;

/// Converts degrees to radians.
function DegToRad(deg) { return (deg / 180.0 * pi) }

/// Converts radians to degrees.
function RadToDeg(rad) { return (rad / pi * 180.0) }

/// Converts a longitude to meter using the mercator projection
function ToMercatorX(lon) {
    return sm_a * DegToRad(lon);
}

/// Converts a longitude to meter using the mercator projection
function FromMercatorX(x) {
    return RadToDeg(x / sm_a);
}

/// Converts a latitude to meter using the mercator projection
function ToMercatorY(lat) {

    if (lat > 89.5) {
        lat = 89.5;
    }

    if (lat < -89.5) {
        lat = -89.5;
    }

    var temp = sm_b / sm_a;
    var es = 1.0 - (temp * temp);
    var eccent = Math.sqrt(es);
    var phi = DegToRad(lat);
    var sinphi = Math.sin(phi);
    var con = eccent * sinphi;
    var com = .5 * eccent;
    con = Math.pow((1.0-con)/(1.0+con), com);
    var ts = Math.tan(.5 * (Math.PI*0.5 - phi))/con;
    var y = 0 - sm_a * Math.log(ts);

    return y;
}

/// Converts a longitude to meter using the mercator projection
function FromMercatorY(y) {

    var temp = sm_b / sm_a;
    var e = Math.sqrt(1.0 - (temp * temp));
    var lat = RadToDeg(PjPhi2(Math.exp( 0 - (y / sm_a)), e));

    return lat;
}

/// Taken from http://wiki.openstreetmap.org/wiki/Mercator - used in reverse mercator function
function PjPhi2(ts, e) {

    var N_ITER=15;
    var HALFPI=Math.PI/2;

    var TOL=0.0000000001;
    var eccnth, phi, con, dphi;
    var i;
    var eccnth = .5 * e;
    phi = HALFPI - 2. * Math.atan (ts);
    i = N_ITER;

    do {
        con = e * Math.sin (phi);
        dphi = HALFPI - 2. * Math.atan (ts * Math.pow((1. - con) / (1. + con), eccnth)) - phi;
        phi += dphi;

    } while ( Math.abs(dphi)>TOL && --i);

    return phi;
}

/// Wrapper: calculates meter in mercator from a lon and lat pair
function LatLonToMercatorXY(lat, lon) {
    return {x: ToMercatorX(lon), y: ToMercatorY(lat)};
}

/// Wrapper: calculates lat and lon using the mercator projection from an x and y pair
function MercatorXYToLatLon(x, y) {
    return {lng: FromMercatorX(x), lat: FromMercatorY(y)};
}

/// Get the horizontal circumference of the earth at a specifi latitude
/// lat in degrees not needed, utmconvert takes care of correct converting
function get_circumference_at_lat(lat) {

    // missing distance of the latitude to the nearest pole, in degrees
    var lat_to_pole = null;

    if (lat < 0.0) {
        lat_to_pole = -90.0 - lat;
    } else {
        lat_to_pole = 90.0 - lat;
    }

    lat = deg_to_rad(lat);

    var f1 = Math.pow((Math.pow(WGS_ELLIPSOID.a, 2) * Math.cos(lat)), 2);
    var f2 = Math.pow((Math.pow(WGS_ELLIPSOID.b, 2) * Math.sin(lat)), 2);
    var f3 = Math.pow((WGS_ELLIPSOID.a * Math.cos(lat)), 2);
    var f4 = Math.pow((WGS_ELLIPSOID.b * Math.sin(lat)), 2);

    // radius of center of earth to surface, on ellipsoid
    var radius_earth_center =  Math.sqrt((f1 + f2) / (f3 + f4));
    var radius_lat_horizontal = Math.sin(deg_to_rad(lat_to_pole)) * radius_earth_center;
    var circumference_horizontal = 2.0 * Math.PI * radius_lat_horizontal;

    return circumference_horizontal;
}

/// Converts degrees to radians
function deg_to_rad(deg)
{
    return (deg * Math.PI  / 180.0);
}


/// Converts radians to degrees
function rad_to_deg(rad)
{
  return (rad * 180.0 / Math.PI);
}

/*

utmconvert.js, available at https://github.com/urbanetic/utm-converter

The MIT License (MIT)

Copyright (c) 2014 Urbanetic

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/

var pi = Math.PI;

// Ellipsoid model constants (actual values here are for WGS84)
var sm_a = 6378137.0;
var sm_b = 6356752.314;
var sm_EccSquared = 6.69437999013e-03;

var UTMScaleFactor = 0.9996;
var FALSE_NORTHING = 10000000.0;
var FALSE_EASTING = 500000.0;

/// Computes the ellipsoidal distance from the equator to a point at a
/// given latitude
/// phi - Latitude of the point, in radians
/// Returns:  The ellipsoidal distance of the point from the equator, in meters
function ArcLengthOfMeridian(phi) {

    var alpha, beta, gamma, delta, epsilon, n;
    var result;

    /* Precalculate n */
    n = (sm_a - sm_b) / (sm_a + sm_b);

    /* Precalculate alpha */
    alpha = ((sm_a + sm_b) / 2.0)
    * (1.0 + (Math.pow (n, 2.0) / 4.0) + (Math.pow (n, 4.0) / 64.0));

    /* Precalculate beta */
    beta = (-3.0 * n / 2.0) + (9.0 * Math.pow (n, 3.0) / 16.0)
    + (-3.0 * Math.pow (n, 5.0) / 32.0);

    /* Precalculate gamma */
    gamma = (15.0 * Math.pow (n, 2.0) / 16.0)
    + (-15.0 * Math.pow (n, 4.0) / 32.0);

    /* Precalculate delta */
    delta = (-35.0 * Math.pow (n, 3.0) / 48.0)
    + (105.0 * Math.pow (n, 5.0) / 256.0);

    /* Precalculate epsilon */
    epsilon = (315.0 * Math.pow (n, 4.0) / 512.0);

    /* Now calculate the sum of the series and return */
    result = alpha
    * (phi + (beta * Math.sin (2.0 * phi))
        + (gamma * Math.sin (4.0 * phi))
        + (delta * Math.sin (6.0 * phi))
        + (epsilon * Math.sin (8.0 * phi)));

    return result;
}

// Determines the central meridian for the given UTM zone.
// zone - An integer value designating the UTM zone, range [1,60].
function UTMCentralMeridian(zone) { return DegToRad(-183.0 + (zone * 6.0)); }

/// Computes the footpoint latitude for use in converting transverse
/// Mercator coordinates to ellipsoidal coordinates.
/// y - The UTM northing coordinate, in meters.
/// Returns: The footpoint latitude, in radians.
function FootpointLatitude(y) {

    var y_, alpha_, beta_, gamma_, delta_, epsilon_, n;
    var result;

    /* Precalculate n (Eq. 10.18) */
    n = (sm_a - sm_b) / (sm_a + sm_b);

    /* Precalculate alpha_ (Eq. 10.22) */
    /* (Same as alpha in Eq. 10.17) */
    alpha_ = ((sm_a + sm_b) / 2.0)
    * (1 + (Math.pow (n, 2.0) / 4) + (Math.pow (n, 4.0) / 64));

    /* Precalculate y_ (Eq. 10.23) */
    y_ = y / alpha_;

    /* Precalculate beta_ (Eq. 10.22) */
    beta_ = (3.0 * n / 2.0) + (-27.0 * Math.pow (n, 3.0) / 32.0)
    + (269.0 * Math.pow (n, 5.0) / 512.0);

    /* Precalculate gamma_ (Eq. 10.22) */
    gamma_ = (21.0 * Math.pow (n, 2.0) / 16.0)
    + (-55.0 * Math.pow (n, 4.0) / 32.0);

    /* Precalculate delta_ (Eq. 10.22) */
    delta_ = (151.0 * Math.pow (n, 3.0) / 96.0)
    + (-417.0 * Math.pow (n, 5.0) / 128.0);

    /* Precalculate epsilon_ (Eq. 10.22) */
    epsilon_ = (1097.0 * Math.pow (n, 4.0) / 512.0);

    /* Now calculate the sum of the series (Eq. 10.21) */
    result = y_ + (beta_ * Math.sin (2.0 * y_))
    + (gamma_ * Math.sin (4.0 * y_))
    + (delta_ * Math.sin (6.0 * y_))
    + (epsilon_ * Math.sin (8.0 * y_));

    return result;
}

/// Converts a latitude/longitude pair to x and y coordinates in the
/// Transverse Mercator projection.  Note that Transverse Mercator is not
/// the same as UTM; a scale factor is required to convert between them.
/// phi - Latitude of the point, in radians.
/// lambda - Longitude of the point, in radians.
/// lambda0 - Longitude of the central meridian to be used, in radians.
/// Returns: [x, y] (easting, northing) - a 2-element array containing the
/// x and y coordinates
function MapLatLonToXY(phi, lambda, lambda0) {

    var N, nu2, ep2, t, t2, l;
    var l3coef, l4coef, l5coef, l6coef, l7coef, l8coef;
    var tmp;

    /* Precalculate ep2 */
    ep2 = (Math.pow (sm_a, 2.0) - Math.pow (sm_b, 2.0)) / Math.pow (sm_b, 2.0);

    /* Precalculate nu2 */
    nu2 = ep2 * Math.pow (Math.cos (phi), 2.0);

    /* Precalculate N */
    N = Math.pow (sm_a, 2.0) / (sm_b * Math.sqrt (1 + nu2));

    /* Precalculate t */
    t = Math.tan (phi);
    t2 = t * t;
    tmp = (t2 * t2 * t2) - Math.pow (t, 6.0);

    /* Precalculate l */
    l = lambda - lambda0;

    /* Precalculate coefficients for l**n in the equations below
    so a normal human being can read the expressions for easting
    and northing
    -- l**1 and l**2 have coefficients of 1.0 */
    l3coef = 1.0 - t2 + nu2;

    l4coef = 5.0 - t2 + 9 * nu2 + 4.0 * (nu2 * nu2);

    l5coef = 5.0 - 18.0 * t2 + (t2 * t2) + 14.0 * nu2
    - 58.0 * t2 * nu2;

    l6coef = 61.0 - 58.0 * t2 + (t2 * t2) + 270.0 * nu2
    - 330.0 * t2 * nu2;

    l7coef = 61.0 - 479.0 * t2 + 179.0 * (t2 * t2) - (t2 * t2 * t2);

    l8coef = 1385.0 - 3111.0 * t2 + 543.0 * (t2 * t2) - (t2 * t2 * t2);

    /* Calculate easting (x) */
    var easting = N * Math.cos (phi) * l
    + (N / 6.0 * Math.pow (Math.cos (phi), 3.0) * l3coef * Math.pow (l, 3.0))
    + (N / 120.0 * Math.pow (Math.cos (phi), 5.0) * l5coef * Math.pow (l, 5.0))
    + (N / 5040.0 * Math.pow (Math.cos (phi), 7.0) * l7coef * Math.pow (l, 7.0));

    /* Calculate northing (y) */
    var northing = ArcLengthOfMeridian(phi)
    + (t / 2.0 * N * Math.pow (Math.cos (phi), 2.0) * Math.pow (l, 2.0))
    + (t / 24.0 * N * Math.pow (Math.cos (phi), 4.0) * l4coef * Math.pow (l, 4.0))
    + (t / 720.0 * N * Math.pow (Math.cos (phi), 6.0) * l6coef * Math.pow (l, 6.0))
    + (t / 40320.0 * N * Math.pow (Math.cos (phi), 8.0) * l8coef * Math.pow (l, 8.0));

    return {x: easting, y: northing};
}


// Converts x and y coordinates in the Transverse Mercator projection to
// a latitude/longitude pair.  Note that Transverse Mercator is not
// the same as UTM; a scale factor is required to convert between them.
// x - The easting of the point, in meters.
// y - The northing of the point, in meters.
// lambda0 - Longitude of the central meridian to be used, in radians.
//
// {lat, lng} as an object
function MapXYToLatLon(x, y, lambda0) {

    var phif, Nf, Nfpow, nuf2, ep2, tf, tf2, tf4, cf;
    var x1frac, x2frac, x3frac, x4frac, x5frac, x6frac, x7frac, x8frac;
    var x2poly, x3poly, x4poly, x5poly, x6poly, x7poly, x8poly;

    /* Get the value of phif, the footpoint latitude. */
    phif = FootpointLatitude (y);

    /* Precalculate ep2 */
    ep2 = (Math.pow (sm_a, 2.0) - Math.pow (sm_b, 2.0))
    / Math.pow (sm_b, 2.0);

    /* Precalculate cos (phif) */
    cf = Math.cos (phif);

    /* Precalculate nuf2 */
    nuf2 = ep2 * Math.pow (cf, 2.0);

    /* Precalculate Nf and initialize Nfpow */
    Nf = Math.pow (sm_a, 2.0) / (sm_b * Math.sqrt (1 + nuf2));
    Nfpow = Nf;

    /* Precalculate tf */
    tf = Math.tan (phif);
    tf2 = tf * tf;
    tf4 = tf2 * tf2;

    /* Precalculate fractional coefficients for x**n in the equations
    below to simplify the expressions for latitude and longitude. */
    x1frac = 1.0 / (Nfpow * cf);

    Nfpow *= Nf;   /* now equals Nf**2) */
    x2frac = tf / (2.0 * Nfpow);

    Nfpow *= Nf;   /* now equals Nf**3) */
    x3frac = 1.0 / (6.0 * Nfpow * cf);

    Nfpow *= Nf;   /* now equals Nf**4) */
    x4frac = tf / (24.0 * Nfpow);

    Nfpow *= Nf;   /* now equals Nf**5) */
    x5frac = 1.0 / (120.0 * Nfpow * cf);

    Nfpow *= Nf;   /* now equals Nf**6) */
    x6frac = tf / (720.0 * Nfpow);

    Nfpow *= Nf;   /* now equals Nf**7) */
    x7frac = 1.0 / (5040.0 * Nfpow * cf);

    Nfpow *= Nf;   /* now equals Nf**8) */
    x8frac = tf / (40320.0 * Nfpow);

    /* Precalculate polynomial coefficients for x**n.
    -- x**1 does not have a polynomial coefficient. */
    x2poly = -1.0 - nuf2;

    x3poly = -1.0 - 2 * tf2 - nuf2;

    x4poly = 5.0 + 3.0 * tf2 + 6.0 * nuf2 - 6.0 * tf2 * nuf2
    - 3.0 * (nuf2 *nuf2) - 9.0 * tf2 * (nuf2 * nuf2);

    x5poly = 5.0 + 28.0 * tf2 + 24.0 * tf4 + 6.0 * nuf2 + 8.0 * tf2 * nuf2;

    x6poly = -61.0 - 90.0 * tf2 - 45.0 * tf4 - 107.0 * nuf2
    + 162.0 * tf2 * nuf2;

    x7poly = -61.0 - 662.0 * tf2 - 1320.0 * tf4 - 720.0 * (tf4 * tf2);

    x8poly = 1385.0 + 3633.0 * tf2 + 4095.0 * tf4 + 1575 * (tf4 * tf2);

    /* Calculate latitude */
    var c_lat = phif + x2frac * x2poly * (x * x)
    + x4frac * x4poly * Math.pow (x, 4.0)
    + x6frac * x6poly * Math.pow (x, 6.0)
    + x8frac * x8poly * Math.pow (x, 8.0);

    /* Calculate longitude */
    var c_lon = lambda0 + x1frac * x
    + x3frac * x3poly * Math.pow (x, 3.0)
    + x5frac * x5poly * Math.pow (x, 5.0)
    + x7frac * x7poly * Math.pow (x, 7.0);

    return {lat: c_lat, lng: c_lon};
}

// Converts a latitude/longitude pair to x and y coordinates in the
// Universal Transverse Mercator projection.
// Returns Object{x, y, zone}
function LatLonToUTMXY(lat, lon) {

    var zone = Math.floor((lon + 180) / 6) + 1;
    console.assert(zone > 0);
    var xy = MapLatLonToXY(DegToRad(lat), DegToRad(lon), UTMCentralMeridian(zone));

    /* Adjust easting and northing for UTM system. */
    xy.x = (xy.x * UTMScaleFactor) + FALSE_EASTING;
    xy.y = (xy.y * UTMScaleFactor) + FALSE_NORTHING;

    return {x: xy.x, y: xy.y, zone: zone};
}

// Converts x and y coordinates in the Universal Transverse Mercator
// projection to a latitude/longitude pair.
function UTMXYToLatLon(x, y, zone) {

    x = (x - FALSE_EASTING) / UTMScaleFactor;
    y = (y - FALSE_NORTHING) / UTMScaleFactor;

    var latlon = MapXYToLatLon(x, y, UTMCentralMeridian(zone));

    return { lat: RadToDeg(latlon.lat), lng: RadToDeg(latlon.lng) };
}


/// Returns the initial coordinates for drawing the map
///
/// NOTE: the projection is switched from UTM (for smaller scales)
/// to an accurate mercator projection (for bigger scales)
/// The breaking point is 600 km (which is enough to fit in one UTM zone)
function calculate_coords(width, height, scale, lon, lat) {

    var total_map_meter_vert = height * (scale / 1000);
    var total_map_meter_horz = width * (scale / 1000);
    
    if (total_map_meter_horz > BREAKING_POINT_METER) {
        // use regular mercator projection (not web mercator!) for bigger scales
        var mercator_result = LatLonToMercatorXY(lat, lon);
    
        var north_utm = mercator_result.y + (total_map_meter_vert / 2);
        var south_utm = mercator_result.y - (total_map_meter_vert / 2);
        var east_utm = mercator_result.x + (total_map_meter_horz / 2);
        var west_utm = mercator_result.x - (total_map_meter_horz / 2);
    
        var north_east_deg = MercatorXYToLatLon(east_utm, north_utm);
        var south_west_deg = MercatorXYToLatLon(west_utm, south_utm);
    
        return {
            coords: [north_east_deg, south_west_deg],
            projection: "mercator"
        };
    } else {
    
        // use UTM projection for smaller scales
        var utm_result = LatLonToUTMXY(lat, lon);
    
        var north_utm = utm_result.y + (total_map_meter_vert / 2);
        var south_utm = utm_result.y - (total_map_meter_vert / 2);
        var east_utm = utm_result.x + (total_map_meter_horz / 2);
        var west_utm = utm_result.x - (total_map_meter_horz / 2);
    
        var north_east_deg = UTMXYToLatLon(east_utm, north_utm, utm_result.zone);
        var south_west_deg = UTMXYToLatLon(west_utm, south_utm, utm_result.zone);
    
        return {
            coords: [north_east_deg, south_west_deg],
            projection: "utm"
        };
    }
    }