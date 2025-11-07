import type { CodegenConfig } from "@graphql-codegen/cli";
import "dotenv/config";

const config: CodegenConfig = {
  schema: "schema.graphql",
  documents: ["./src/**/*.js", "./src/**/*.ts"],
  ignoreNoDocuments: false, // for better experience with the watcher
  generates: {
    "./generated/": {
      preset: "client",
      plugins: ["typescript"],
      config: {
        useTypeImports: true,
      },
    },
  },
};

export default config;
