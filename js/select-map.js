

var CDN_URL = config.dataset.cdnUrl;
var SERVER_URL = config.dataset.serverUrl;
var DEFAULT_LNG = parseFloat(config.dataset.defaultLng);
var DEFAULT_LAT = parseFloat(config.dataset.defaultLat);
var DEFAULT_ZOOMLEVEL = parseInt(config.dataset.defaultZoomlevel);
var APP_LANG = config.dataset.appLang;
var DEFAULT_WIDTH_MM = parseInt(config.dataset.defaultWidthMm);
var DEFAULT_HEIGHT_MM = parseInt(config.dataset.defaultHeightMm);
var NO_RESULTS_LANG =  config.dataset.noResults;
var SHOW_CHAT_LANG =  config.dataset.showChat;
var HIDE_CHAT_LANG =  config.dataset.hideChat;
var MAP_EXISTS = config.dataset.mapExists;
var DEFAULT_SVG = config.dataset.defaultSvg;

var BREAKING_POINT_METER = 150000;
var ROUNDING_FACTOR = 100000000;

var drag_icon_normal_src = CDN_URL + '/img/select-map/background-marker-drag-sm.webp';
var drag_icon_normal = L.icon({
    iconUrl: drag_icon_normal_src,
    iconSize: [20, 20],
});

var drag_icon_middle_src = CDN_URL + '/img/select-map/background-marker-middle.webp';
var drag_icon_middle = L.icon({
    iconUrl: drag_icon_middle_src,
    iconSize: [30, 30],
});

var icon_svgs = {};
icon_svgs[DEFAULT_SVG] = L.divIcon({
  html: window.atob(DEFAULT_SVG),
  className: "leaflet-icon-0",
  iconSize: [40, 40],
  iconAnchor: [20, 40],
});
var markers_in_map = [];
var lines_in_map = [];

var map_div = document.getElementById("mapid");
var map_width_input = document.getElementById("width");
var map_height_input = document.getElementById("height");
var map_scale_select = document.getElementById("scale");
var map_latitude_input = document.getElementById("latitude");
var map_longitude_input = document.getElementById("longitude");
var map_projection_input = document.getElementById("projection");

var map_extent_color = "#1549c0";
var map_extent = null;
var map_handle_center = null;
var map_handle_top_left = null;
var map_handle_top_right = null;
var map_handle_bottom_left = null;
var map_handle_bottom_right = null;

var old_center_lat_lng = null;
var old_top_left_lat_lng = null;
var old_top_right_lat_lng = null;
var old_bottom_right_lat_lng = null;
var old_bottom_left_lat_lng = null;

/// Returns an approximate scale for this zoomlevel
function get_approximate_scale_for_zoomlevel(level) {

    // we need to determine the size at zoom level 2 at the maximum scale
    // and then just divide every sub-zoomlevel by 2 to get the scale

    var base_scale = 100000000; // scale at zoomlevel 2, 100 million
    var rec_zoom = level - 1;
    var rect_scale = base_scale / Math.pow(2, rec_zoom);
    return rect_scale;
}

// Returns a rounded scale for the zoomlevel
function get_scale_for_zoom_level(level) {
    var approximate = get_approximate_scale_for_zoomlevel(level);

    if (approximate < 10000) { return 5000; }
    else if (approximate < 15000) { return 10000; }
    else if (approximate < 20000) { return 15000; }
    else if (approximate < 25000) { return 20000; }
    else if (approximate < 30000) { return 25000; }
    else if (approximate < 35000) { return 30000; }
    else if (approximate < 40000) { return 35000; }
    else if (approximate < 50000) { return 40000; }
    else if (approximate < 75000) { return 50000; }
    else if (approximate < 100000) { return 75000; }
    else if (approximate < 150000) { return 100000; }
    else if (approximate < 200000) { return 150000; }
    else if (approximate < 300000) { return 200000; }
    else if (approximate < 400000) { return 300000; }
    else if (approximate < 500000) { return 400000; }
    else if (approximate < 750000) { return 500000; }
    else if (approximate < 1000000) { return 750000; }
    else if (approximate < 1500000) { return 1000000; }
    else if (approximate < 2000000) { return 1500000; }
    else if (approximate < 2500000) { return 2000000; }
    else if (approximate < 5000000) { return 2500000; }
    else if (approximate < 10000000) { return 5000000; }
    else { return 10000000; }
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

// Creates the map from the current given extent
function create_map_from_extent() {
    var map_center = map.getCenter();
    map_width_input.value = DEFAULT_WIDTH_MM;
    map_height_input.value = DEFAULT_HEIGHT_MM;
    map_longitude_input.value = Math.round(map_center.lng * ROUNDING_FACTOR) / ROUNDING_FACTOR;
    map_latitude_input.value = Math.round(map_center.lat * ROUNDING_FACTOR) / ROUNDING_FACTOR;
    map_scale_select.value = get_scale_for_zoom_level(map.getZoom());
}

// Removes the map from the leaflet map
function remove_map() {
    if (map_extent !== null) { map_extent.removeFrom(map); }
    if (map_handle_center !== null) { map_handle_center.removeFrom(map); }
    if (map_handle_top_left !== null) { map_handle_top_left.removeFrom(map); }
    if (map_handle_top_right !== null) { map_handle_top_right.removeFrom(map); }
    if (map_handle_bottom_left !== null) { map_handle_bottom_left.removeFrom(map); }
    if (map_handle_bottom_right !== null) { map_handle_bottom_right.removeFrom(map); }
    map_extent = null;
    map_handle_center = null;
    map_handle_top_left = null;
    map_handle_top_right = null;
    map_handle_bottom_left = null;
    map_handle_bottom_right = null;
}

// Removes and re-calculates and re-adds the map
function add_map() {
    remove_map();

    var width = parseInt(map_width_input.value);
    var height = parseInt(map_height_input.value);
    var scale = parseInt(map_scale_select.value);
    var lon = parseFloat(map_longitude_input.value);
    var lat = parseFloat(map_latitude_input.value);

    var rect = calculate_coords(width, height, scale, lon, lat);
    map_projection_input.value = rect.projection;

    map_extent = L.rectangle(rect.coords, {color: map_extent_color, weight: 2}).addTo(map);
    map_handle_center = L.marker(map_extent.getCenter(), { draggable: true, icon: drag_icon_middle });
    map_handle_top_left = L.marker([rect.coords[0].lat, rect.coords[1].lng], { draggable: true, icon: drag_icon_normal });
    map_handle_top_right = L.marker([rect.coords[0].lat, rect.coords[0].lng], { draggable: true, icon: drag_icon_normal });
    map_handle_bottom_left = L.marker([rect.coords[1].lat, rect.coords[1].lng], { draggable: true, icon: drag_icon_normal });
    map_handle_bottom_right = L.marker([rect.coords[1].lat, rect.coords[0].lng], { draggable: true, icon: drag_icon_normal });

    map_handle_center.on('drag', function(){ drag_handle("center") });
    map_handle_top_left.on('drag', function(){ drag_handle("top_left") });
    map_handle_top_right.on('drag', function(){ drag_handle("top_right") });
    map_handle_bottom_left.on('drag', function(){ drag_handle("bottom_left") });
    map_handle_bottom_right.on('drag', function(){ drag_handle("bottom_right") });

    map_handle_center.addTo(map);
    map_handle_top_left.addTo(map);
    map_handle_top_right.addTo(map);
    map_handle_bottom_left.addTo(map);
    map_handle_bottom_right.addTo(map);

    update_lat_lng();
}

function update_lat_lng() {
    old_center_lat_lng = map_handle_center.getLatLng();
    old_top_left_lat_lng = map_handle_top_left.getLatLng();
    old_top_right_lat_lng = map_handle_top_right.getLatLng();
    old_bottom_right_lat_lng = map_handle_bottom_right.getLatLng();
    old_bottom_left_lat_lng = map_handle_bottom_left.getLatLng();
    map_extent.setBounds([
        map_handle_top_left.getLatLng(),
        map_handle_bottom_right.getLatLng(),
    ]);
}

// Recalculates the map without removing it
function modify_recalculate_map() {

    var width = parseInt(map_width_input.value);
    if (isNaN(width)) {
        width = DEFAULT_WIDTH_MM;
    }
    var height = parseInt(map_height_input.value);
    if (isNaN(height)) {
        height = DEFAULT_HEIGHT_MM;
    }
    var scale = parseInt(map_scale_select.value);
    if (isNaN(scale)) {
        scale = get_scale_for_zoom_level(DEFAULT_ZOOMLEVEL);
    }
    var lon = parseFloat(map_longitude_input.value);
    if (isNaN(lon)) {
        lon = DEFAULT_LNG;
    }
    var lat = parseFloat(map_latitude_input.value);
    if (isNaN(lat)) {
        lat = DEFAULT_LAT;
    }

    var rect = calculate_coords(width, height, scale, lon, lat);
    map_projection_input.value = rect.projection;

    map_handle_center.setLatLng(map_extent.getCenter());
    map_handle_top_left.setLatLng({lat: rect.coords[0].lat, lng: rect.coords[1].lng});
    map_handle_top_right.setLatLng({lat: rect.coords[0].lat, lng: rect.coords[0].lng});
    map_handle_bottom_left.setLatLng({lat: rect.coords[1].lat, lng: rect.coords[1].lng});
    map_handle_bottom_right.setLatLng({lat: rect.coords[1].lat, lng: rect.coords[0].lng});

    update_lat_lng();
}

function drag_handle(id) {
    switch (id) {
        case "center":

            var center_lat_lng = map_handle_center.getLatLng();

            var width = parseInt(map_width_input.value);
            var height = parseInt(map_height_input.value);
            var scale = parseInt(map_scale_select.value);

            var rect = calculate_coords(width, height, scale, center_lat_lng.lng, center_lat_lng.lat);
            map_projection_input.value = rect.projection;

            map_handle_top_left.setLatLng({ lat: rect.coords[0].lat, lng: rect.coords[1].lng});
            map_handle_top_right.setLatLng({ lat: rect.coords[0].lat, lng: rect.coords[0].lng});
            map_handle_bottom_left.setLatLng({ lat: rect.coords[1].lat, lng: rect.coords[1].lng});
            map_handle_bottom_right.setLatLng({ lat: rect.coords[1].lat, lng: rect.coords[0].lng});

            break;
        case "top_left":

            var top_left_lat_lng = map_handle_top_left.getLatLng();

            map_handle_top_right.setLatLng({ lat: top_left_lat_lng.lat, lng: old_top_right_lat_lng.lng });
            map_handle_bottom_left.setLatLng({ lat: old_bottom_left_lat_lng.lat, lng: top_left_lat_lng.lng });

            break;
        case "top_right":

            var top_right_lat_lng = map_handle_top_right.getLatLng();

            map_handle_top_left.setLatLng({ lat: top_right_lat_lng.lat, lng: old_top_left_lat_lng.lng });
            map_handle_bottom_right.setLatLng({ lat: old_bottom_right_lat_lng.lat, lng: top_right_lat_lng.lng });

            break;
        case "bottom_right":

            var bottom_right_lat_lng = map_handle_bottom_right.getLatLng();

            map_handle_top_right.setLatLng({ lat: old_top_right_lat_lng.lat, lng: bottom_right_lat_lng.lng });
            map_handle_bottom_left.setLatLng({ lat: bottom_right_lat_lng.lat, lng: old_bottom_left_lat_lng.lng });

            break;
        case "bottom_left":

            var bottom_left_lat_lng = map_handle_bottom_left.getLatLng();

            map_handle_top_left.setLatLng({ lat: old_top_left_lat_lng.lat, lng: bottom_left_lat_lng.lng });
            map_handle_bottom_right.setLatLng({ lat: bottom_left_lat_lng.lat, lng: old_bottom_right_lat_lng.lng });

            break;
        default:
            break;
    }

    map_extent.setBounds([
        map_handle_top_left.getLatLng(),
        map_handle_bottom_right.getLatLng(),
    ]);
    map_handle_center.setLatLng(map_extent.getCenter());

    update_lat_lng();

    var map_handle_center_lat_lng = map_handle_center.getLatLng();

    map_longitude_input.value = Math.round(map_handle_center_lat_lng.lng * ROUNDING_FACTOR) / ROUNDING_FACTOR;
    map_latitude_input.value = Math.round(map_handle_center_lat_lng.lat * ROUNDING_FACTOR) / ROUNDING_FACTOR;

    if (!(id === "center" && map_projection_input.value === "utm")) {
        recalculate_width_height();
    }
}

function recalculate_width_height() {

    var projection = map_projection_input.value;
    var scale = parseInt(map_scale_select.value);

    switch (projection) {
        case "utm":

            var top_left_lat_lng = map_handle_top_left.getLatLng();
            var top_right_lat_lng = map_handle_top_right.getLatLng();
            var bottom_left_lat_lng = map_handle_bottom_left.getLatLng();

            var top_left_utm = LatLonToUTMXY(top_left_lat_lng.lat, top_left_lat_lng.lng);
            var top_right_utm = LatLonToUTMXY(top_right_lat_lng.lat, top_right_lat_lng.lng);
            var bottom_left_utm = LatLonToUTMXY(bottom_left_lat_lng.lat, bottom_left_lat_lng.lng);

            var new_width_meter = top_right_utm.x - top_left_utm.x;
            var new_height_meter = top_left_utm.y - bottom_left_utm.y;

            map_width_input.value = Math.round(Math.abs(new_width_meter) / scale * 1000.0);
            map_height_input.value = Math.round(Math.abs(new_height_meter) / scale * 1000.0);

            break;
        case "mercator":

            var top_left_lat_lng = map_handle_top_left.getLatLng();
            var top_right_lat_lng = map_handle_top_right.getLatLng();
            var bottom_left_lat_lng = map_handle_bottom_left.getLatLng();

            var top_left_merc = LatLonToMercatorXY(top_left_lat_lng.lat, top_left_lat_lng.lng);
            var top_right_merc = LatLonToMercatorXY(top_right_lat_lng.lat, top_right_lat_lng.lng);
            var bottom_left_merc = LatLonToMercatorXY(bottom_left_lat_lng.lat, bottom_left_lat_lng.lng);

            var new_width_meter = top_right_merc.x - top_left_merc.x;
            var new_height_meter = top_left_merc.y - bottom_left_merc.y;

            map_width_input.value = Math.round(Math.abs(new_width_meter) / scale * 1000.0);
            map_height_input.value = Math.round(Math.abs(new_height_meter) / scale * 1000.0);

            break;
        default:
            break;
    }
}

function map_edit_width(event) {
    if (!is_a_number(event)) {
        event.preventDefault();
        return false;
    }
    modify_recalculate_map();
}

function map_edit_height(event) {
    if (!is_a_number(event)) {
        event.preventDefault();
        return false;
    }
    modify_recalculate_map();
}

function map_edit_scale(event) {
    modify_recalculate_map();
}

/// Should be called on key down so that the event is cancelable
function prevent_number_input(event) {
    if (!is_a_number(event)) {
        event.preventDefault();
        return false;
    }
}

/// Look if array contains a value
function array_contains(haystack, needle) {

    var index = -1;

    for(var i = -1; i < haystack.length; i++) {
        var item = haystack[i];

        if(item === needle) {
            index = i;
            break;
        }
    }

    return (index > -1);
};

/// helper function to only allow numbers, returns a boolean to see if
/// the input is allowed in a number field
function is_a_number(e) {

    // NOTE: Ctrl+V is not allowed, since it could lead to pasting text + further errors
    // (e.keyCode == 86 && (e.ctrlKey === true || e.metaKey === true)) ||

    var allowed_keys = [46, 8, 9, 27, 13, 110];
    if (array_contains(allowed_keys, e.keyCode) ||
        // Allow: Ctrl+A
        (e.keyCode == 65 && (e.ctrlKey === true || e.metaKey === true)) ||
        // Allow: Ctrl+C
        (e.keyCode == 67 && (e.ctrlKey === true || e.metaKey === true)) ||
        (e.keyCode == 17 && (e.ctrlKey === true || e.metaKey === true)) ||
        // Allow: Ctrl+X
        (e.keyCode == 88 && (e.ctrlKey === true || e.metaKey === true)) ||
        // Allow: home, end, left, right
        (e.keyCode >= 35 && e.keyCode <= 39) ||
        //Allow numbers and numbers + shift key
        ((!e.shiftKey && (e.keyCode >= 48 && e.keyCode <= 57)) || (e.keyCode >= 96 && e.keyCode <= 105))) {
        return true;
    } else {
        return false;
    }
}

if (!MAP_EXISTS) {
    create_map_from_extent();
}
add_map();
modify_recalculate_map();
if (MAP_EXISTS) {
    map.fitBounds(map_extent.getBounds());
    map_handle_center.setLatLng(map_extent.getCenter());
}
