import { useState } from "react";

interface TimelineDot {
  id: string;
  index: number;
  preview: string;
  timestamp: string | null;
}

interface Props {
  dots: TimelineDot[];
  activeId: string | null;
  onDotClick: (id: string) => void;
}

export function TimelineDots({ dots, activeId, onDotClick }: Props) {
  const [hoveredId, setHoveredId] = useState<string | null>(null);

  if (dots.length === 0) return null;

  return (
    <div className="absolute right-1.5 top-20 bottom-28 z-10 pointer-events-none">
      <div className="relative h-full py-4 pointer-events-auto w-3">
        {dots.map((dot, i) => {
          const isActive = dot.id === activeId;
          const isHovered = dot.id === hoveredId;
          // Evenly distribute dots across full height by percentage
          const position = dots.length === 1 ? 50 : (i / (dots.length - 1)) * 100;

          return (
            <div
              key={dot.id}
              className="absolute right-0 -translate-y-1/2"
              style={{ top: `${position}%` }}
            >
              <button
                onClick={() => onDotClick(dot.id)}
                onMouseEnter={() => setHoveredId(dot.id)}
                onMouseLeave={() => setHoveredId(null)}
                className={`w-2 h-2 rounded-full transition-all duration-200 block ${
                  isActive
                    ? "bg-primary scale-150 shadow-sm shadow-primary/30"
                    : "bg-muted-foreground/30 hover:bg-muted-foreground/60 hover:scale-125"
                }`}
                aria-label={dot.preview}
              />
              {isHovered && (
                <div className="absolute right-full mr-3 top-1/2 -translate-y-1/2 pointer-events-none">
                  <div className="relative bg-card border border-border rounded-md px-3 py-1.5 shadow-lg text-xs text-foreground max-w-[16rem]">
                    <p className="truncate">{dot.preview}</p>
                    {dot.timestamp && (
                      <p className="text-muted-foreground text-[10px] mt-0.5">{dot.timestamp}</p>
                    )}
                    {/* Arrow pointing right */}
                    <div className="absolute right-0 top-1/2 -translate-y-1/2 translate-x-[3px]">
                      <div className="w-1.5 h-1.5 bg-card border-r border-t border-border rotate-45" />
                    </div>
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
