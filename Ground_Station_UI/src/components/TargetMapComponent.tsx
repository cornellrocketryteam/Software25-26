import { MapContainer, TileLayer, Marker, Popup, Polyline, useMap } from "react-leaflet";
import L from "leaflet";
import "leaflet/dist/leaflet.css";
import { useEffect } from "react";

export type TargetCoords = {
    lat_1: string;
    lng_1: string;
    lat_2: string;
    lng_2: string;
};

type ParsedTarget = {
    label: string;
    position: [number, number];
    color: string;
};

/**
 * Build a small, color-coded teardrop marker as an inline SVG. Using a divIcon
 * avoids Leaflet's default-marker image paths breaking under the Vite bundler.
 */
const makeMarkerIcon = (color: string, label: string) =>
    L.divIcon({
        className: "",
        html: `
            <div style="position:relative;display:flex;flex-direction:column;align-items:center;">
                <div style="
                    width:22px;height:22px;border-radius:50% 50% 50% 0;
                    background:${color};border:3px solid #000;
                    transform:rotate(-45deg);box-shadow:0 1px 3px rgba(0,0,0,0.4);">
                </div>
                <span style="
                    position:absolute;top:2px;color:#fff;font-family:inherit;
                    font-weight:700;font-size:11px;">${label}</span>
            </div>`,
        iconSize: [22, 30],
        iconAnchor: [11, 30],
        popupAnchor: [0, -30],
    });

/** Keeps the map framed around whatever targets are currently shown. */
function FitBounds({ targets }: { targets: ParsedTarget[] }) {
    const map = useMap();
    useEffect(() => {
        if (targets.length === 0) return;
        if (targets.length === 1) {
            map.setView(targets[0].position, 15);
        } else {
            map.fitBounds(
                L.latLngBounds(targets.map((t) => t.position)),
                { padding: [60, 60], maxZoom: 16 }
            );
        }
    }, [map, targets]);
    return null;
}

type TargetMapProps = {
    coords: TargetCoords | null;
    /** Tailwind height class for the map container. */
    heightClass?: string;
};

/**
 * Renders confirmed recovery target coordinates on an interactive map.
 * Self-contained and prop-driven — it is simple to drop it into any page (or remove it) freely.
 */
export function TargetMap({ coords, heightClass = "h-96" }: TargetMapProps) {
    const targets: ParsedTarget[] = [
        { label: "U", lat: coords?.lat_1, lng: coords?.lng_1, color: "#5A87FF" },
        { label: "D", lat: coords?.lat_2, lng: coords?.lng_2, color: "#FF6B5A" },
    ]
        .map(({ label, lat, lng, color }) => ({
            label,
            color,
            position: [Number(lat), Number(lng)] as [number, number],
        }))
        .filter(
            (t) => !Number.isNaN(t.position[0]) && !Number.isNaN(t.position[1])
        );

    if (targets.length === 0) {
        return (
            <div className={`${heightClass} flex items-center justify-center rounded-2xl border-[3px] border-black bg-white font-inter text-lg text-gray-500`}>
                Confirm coordinates to view targets on the map.
            </div>
        );
    }

    return (
        <div className={`${heightClass} overflow-hidden rounded-2xl border-[3px] border-black`}>
            <MapContainer
                center={targets[0].position}
                zoom={15}
                scrollWheelZoom
                style={{ height: "100%", width: "100%" }}
            >
                <TileLayer
                    attribution='&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors'
                    url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
                />
                {targets.map((t) => (
                    <Marker
                        key={t.label}
                        position={t.position}
                        icon={makeMarkerIcon(t.color, t.label)}
                    >
                        <Popup>
                            <span className="font-inter font-bold">
                                Target {t.label}
                            </span>
                            <br />
                            Lat: {t.position[0]}°
                            <br />
                            Lon: {t.position[1]}°
                        </Popup>
                    </Marker>
                ))}
                {targets.length === 2 && (
                    <Polyline
                        positions={targets.map((t) => t.position)}
                        pathOptions={{ color: "#000", weight: 2, dashArray: "6 6" }}
                    />
                )}
                <FitBounds targets={targets} />
            </MapContainer>
        </div>
    );
}

export default TargetMap;
