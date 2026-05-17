/** @type {import('jest').Config} */
module.exports = {
    preset: "ts-jest",
    testEnvironment: "node",
    roots: ["<rootDir>/tests"],
    transform: {
        "^.+\\.tsx?$": ["ts-jest", {
            tsconfig: "<rootDir>/tsconfig.test.json",
        }],
    },
    moduleNameMapper: {
        "^(\\.\\.?/.*)\\.js$": "$1",
    },
    testMatch: ["**/*.test.ts"],
};
