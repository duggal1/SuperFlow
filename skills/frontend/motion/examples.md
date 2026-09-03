"use client"

import * as motion from "motion/react-client"
import { useState } from "react"

export default function StateAnimations() {
    const [x, setX] = useState(0)
    const [y, setY] = useState(0)
    const [rotate, setRotate] = useState(0)

    return (
        <div id="example">
            <div>
                <motion.div
                    className="box"
                    animate={{ x, y, rotate }}
                    transition={{ type: "spring" }}
                />
            </div>
            <div className="inputs">
                <Input value={x} set={setX}>
                    x
                </Input>
                <Input value={y} set={setY}>
                    y
                </Input>
                <Input value={rotate} set={setRotate} min={-180} max={180}>
                    rotate
                </Input>
            </div>
            <StyleSheet />
        </div>
    )
}

interface InputProps {
    children: string
    value: number
    set: (newValue: number) => void
    min?: number
    max?: number
}

function Input({ value, children, set, min = -200, max = 200 }: InputProps) {
    return (
        <label>
            <code>{children}</code>
            <input
                value={value}
                type="range"
                min={min}
                max={max}
                onChange={(e) => set(parseFloat(e.target.value))}
            />
            <input
                type="number"
                value={value}
                min={min}
                max={max}
                onChange={(e) => set(parseFloat(e.target.value) || 0)}
            />
        </label>
    )
}

/**
 * ==============   Styles   ================
 */

function StyleSheet() {
    return (
        <style>{`
            #example .box {
                width: 200px;
                height: 200px;
                border-radius: 20px;
                border: 5px dotted var(--hue-1);
                pointer-events: none;
            }

            #example {
                display: flex;
                align-items: center;
            }

            #example input {
                accent-color: var(--hue-1);
                font-family: "Geist Mono", monospace;
            }

            #example .inputs {
                display: flex;
                flex-direction: column;
                padding-left: 50px;
            }

            #example label {
                display: flex;
                align-items: center;
                margin: 10px 0;
            }

            #example label code {
                width: 100px;
            }

            #example input[type="number"] {
                border: 0;
                border-bottom: 1px dotted var(--hue-1);
                color: var(--hue-1);
                margin-left: 10px;
            }

            #example input[type="number"]:focus {
                outline: none;
                border-bottom: 2px solid var(--hue-1);
            }

            #example input[type="number"]::-webkit-inner-spin-button {
                -webkit-appearance: none;
            }

            input[type='range']::-webkit-slider-runnable-track {
                height: 10px;
                -webkit-appearance: none;
                background: var(--layer);
                border: 1px solid var(--border);
                border-radius: 10px;
                margin-top: -1px;
            }

            input[type='range']::-webkit-slider-thumb {
                -webkit-appearance: none;
                height: 20px;
                width: 20px;
                border-radius: 50%;
                background: var(--hue-1);
                top: -4px;
                position: relative;
            }
        `}</style>
    )
}
"use client"

import { AnimatePresence } from "motion/react"
import * as motion from "motion/react-client"
import { useState } from "react"

export default function SharedLayoutAnimation() {
    const [selectedTab, setSelectedTab] = useState(tabs[0])

    return (
        <div style={container}>
            <nav style={nav}>
                <ul style={tabsContainer}>
                    {tabs.map((item) => (
                        <motion.li
                            key={item.label}
                            initial={false}
                            animate={{
                                backgroundColor:
                                    item === selectedTab ? "#eee" : "#eee0",
                            }}
                            style={tab}
                            onClick={() => setSelectedTab(item)}
                        >
                            {`${item.icon} ${item.label}`}
                            {item === selectedTab ? (
                                <motion.div
                                    style={underline}
                                    layoutId="underline"
                                    id="underline"
                                />
                            ) : null}
                        </motion.li>
                    ))}
                </ul>
            </nav>
            <main style={iconContainer}>
                <AnimatePresence mode="wait">
                    <motion.div
                        key={selectedTab ? selectedTab.label : "empty"}
                        initial={{ y: 10, opacity: 0 }}
                        animate={{ y: 0, opacity: 1 }}
                        exit={{ y: -10, opacity: 0 }}
                        transition={{ duration: 0.2 }}
                        style={icon}
                    >
                        {selectedTab ? selectedTab.icon : "😋"}
                    </motion.div>
                </AnimatePresence>
            </main>
        </div>
    )
}

/**
 * ==============   Styles   ================
 */

const container: React.CSSProperties = {
    width: 480,
    height: 360,
    maxWidth: "calc(100% - 40px)",
    maxHeight: "calc(100% - 40px)",
    borderRadius: 10,
    background: "white",
    overflow: "hidden",
    boxShadow:
        "0 1px 1px hsl(0deg 0% 0% / 0.075), 0 2px 2px hsl(0deg 0% 0% / 0.075), 0 4px 4px hsl(0deg 0% 0% / 0.075), 0 8px 8px hsl(0deg 0% 0% / 0.075), 0 16px 16px hsl(0deg 0% 0% / 0.075), 0 2px 2px hsl(0deg 0% 0% / 0.075), 0 4px 4px hsl(0deg 0% 0% / 0.075), 0 8px 8px hsl(0deg 0% 0% / 0.075), 0 16px 16px hsl(0deg 0% 0% / 0.075)",
    display: "flex",
    flexDirection: "column",
}

const nav: React.CSSProperties = {
    background: "#fdfdfd",
    padding: "5px 5px 0",
    borderRadius: "10px",
    borderBottomLeftRadius: 0,
    borderBottomRightRadius: 0,
    borderBottom: "1px solid #eeeeee",
    height: 44,
}

const tabsStyles: React.CSSProperties = {
    listStyle: "none",
    padding: 0,
    margin: 0,
    fontWeight: 500,
    fontSize: 14,
}

const tabsContainer: React.CSSProperties = {
    ...tabsStyles,
    display: "flex",
    width: "100%",
}

const tab: React.CSSProperties = {
    ...tabsStyles,
    borderRadius: 5,
    borderBottomLeftRadius: 0,
    borderBottomRightRadius: 0,
    width: "100%",
    padding: "10px 15px",
    position: "relative",
    background: "white",
    cursor: "pointer",
    height: 24,
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    flex: 1,
    minWidth: 0,
    userSelect: "none",
    color: "var(--black)",
}

const underline: React.CSSProperties = {
    position: "absolute",
    bottom: -2,
    left: 0,
    right: 0,
    height: 2,
    background: "var(--accent)",
}

const iconContainer: React.CSSProperties = {
    display: "flex",
    justifyContent: "center",
    alignItems: "center",
    flex: 1,
}

const icon: React.CSSProperties = {
    fontSize: 128,
}

/**
 * ==============   Data   ================
 */

const allIngredients = [
    { icon: "🍅", label: "Tomato" },
    { icon: "🥬", label: "Lettuce" },
    { icon: "🧀", label: "Cheese" },
    { icon: "🥕", label: "Carrot" },
    { icon: "🍌", label: "Banana" },
    { icon: "🫐", label: "Blueberries" },
    { icon: "🥂", label: "Champers?" },
]

const [tomato, lettuce, cheese] = allIngredients
const tabs = [tomato, lettuce, cheese]
