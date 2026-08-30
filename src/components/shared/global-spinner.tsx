export function IOSSpinner({ size = 40, color = "#8E8E93", speed = 1.6 }) {
  const blades = Array.from({ length: 12 });
  const thickness = Math.max(2, size * 0.083);
  const length = size * 0.26;

  return (
    <div
      role="status"
      aria-label="Loading"
      className="relative inline-block shrink-0"
      style={{ width: size, height: size }}
    >
      {blades.map((_, i) => {
        const rotation = i * 30;
        const delay = (i - 12) * (speed / 12);
        return (
          <span
            key={i}
            className="absolute rounded-full ios-spinner-blade"
            style={{
              top: 0,
              left: "50%",
              width: thickness,
              height: length,
              marginLeft: -thickness / 2,
              backgroundColor: color,
              transformOrigin: `50% ${size / 2}px`,
              transform: `rotate(${rotation}deg)`,
              animationDuration: `${speed}s`,
              animationDelay: `${delay}s`,
            }}
          />
        );
      })}

      <style>{`
        @keyframes ios-spinner-fade {
          0%   { opacity: 1; }
          50%  { opacity: 0.22; }
          100% { opacity: 1; }
        }
        .ios-spinner-blade {
          opacity: 0.22;
          animation-name: ios-spinner-fade;
          animation-timing-function: cubic-bezier(0.65, 0, 0.35, 1);
          animation-iteration-count: infinite;
        }
        @media (prefers-reduced-motion: reduce) {
          .ios-spinner-blade {
            animation: none;
            opacity: 0.5;
          }
        }
      `}</style>
    </div>
  );
}
