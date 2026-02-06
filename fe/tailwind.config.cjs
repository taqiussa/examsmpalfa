/** @type {import('tailwindcss').Config} */
module.exports = {
        content: [
                "./index.html",
                "./src/**/*.{js,ts,jsx,tsx}",
                "../templates/**/*.html",
        ],
        theme: {
                extend: {
                        keyframes: {
                                dance: {
                                        "0%, 100%": { transform: "translateX(0)" },
                                        "50%": { transform: "translateX(75%)" },
                                },
                                progress: {
                                        "0%": { transform: "translateX(0) scaleX(0)" },
                                        "40%": { transform: "translateX(0) scaleX(0.4)" },
                                        "100%": { transform: "translateX(100%) scaleX(0.5)" },
                                },
                        },
                        animation: {
                                dance: "dance 1s ease-in-out infinite",
                                progress: "progress 1s infinite linear",
                        },
                        transformOrigin: {
                                "left-right": "0% 50%",
                        },
                        fontFamily: {
                                sans: ["Figtree", "ui-sans-serif", "system-ui"],
                        },
                },
        },
        plugins: [],
};