import { Client, fetchExchange } from "urql";
import { graphql } from "../generated";
import {
  DeleteAllProductsMutation,
  DeleteAllProductsMutationVariables,
  Products_InitialQuery,
  Products_InitialQueryVariables,
  Products_NextQuery,
  Products_NextQueryVariables,
} from "../generated/graphql";

const api_url = "http://localhost:8000/graphql/";

main();

async function main() {
  let token = await get_token(api_url, "[EMAIL HERE]", "[PASSWORD HERE]");
  let client = connect({ api_url: api_url, token: token });

  let all_product_ids: string[] = [];

  let products_first_res = await client.query<
    Products_InitialQuery,
    Products_InitialQueryVariables
  >(fetch_first_gql, {});

  all_product_ids.push(
    ...products_first_res.data?.products?.edges.map((e) => e.node.id)!,
  );

  console.log(all_product_ids);

  let next_cursor = products_first_res.data?.products?.pageInfo?.hasNextPage
    ? products_first_res.data?.products?.pageInfo?.endCursor
    : undefined;

  while (next_cursor) {
    console.log(
      `querying next batch with cursor ...${next_cursor.slice(-3)}, have: ${all_product_ids.length}`,
    );
    let products_next_res = await client.query<
      Products_NextQuery,
      Products_NextQueryVariables
    >(fetch_next_gql, { after: next_cursor });
    all_product_ids.push(
      ...products_next_res.data?.products?.edges.map((e) => e.node.id)!,
    );

    // console.dir(products_next_res, { depth: null });

    next_cursor = products_next_res.data?.products?.pageInfo?.hasNextPage
      ? products_next_res.data?.products?.pageInfo?.endCursor
      : undefined;
  }
  console.log(`Have all ${all_product_ids.length} products`);

  let del_all_res = await client.mutation<
    DeleteAllProductsMutation,
    DeleteAllProductsMutationVariables
  >(delete_all_products, { ids: all_product_ids });
  console.dir(del_all_res, { depth: null })
}

async function get_token(
  api_url: string,
  email: String,
  password: String,
): Promise<string> {
  const query = JSON.stringify({
    query: `
			mutation login($email: String!, $pass: String!) {
				tokenCreate(email: $email, password: $pass){
					token
					refreshToken
				}
			}
			`,
    variables: {
      email: email,
      pass: password,
    },
  });

  const response = await fetch(api_url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: query,
  });
  const json = await response.json();
  // store in local db!
  const token = json.data.tokenCreate.token;
  return token;
}

export function connect(options?: {
  api_url?: string;
  token?: string;
  cache?: boolean;
}): Client {
  const client = new Client({
    url: options?.api_url!,
    exchanges: [fetchExchange],
    requestPolicy: "network-only",
    fetchOptions: {
      headers: { "authorization-bearer": options?.token || "" },
    },
  });
  return client;
}

const fetch_first_gql = graphql(`
  query products_initial {
    products(first: 100, channel: "zakladny") {
      pageInfo {
        hasNextPage
        endCursor
      }
      edges {
        node {
          id
        }
      }
    }
  }
`);

const fetch_next_gql = graphql(`
  query products_next($after: String!) {
    products(first: 100, after: $after, channel: "zakladny") {
      pageInfo {
        hasNextPage
        endCursor
      }
      edges {
        node {
          id
        }
      }
    }
  }
`);

const delete_all_products = graphql(`
  mutation deleteAllProducts($ids: [ID!]!) {
    productBulkDelete(ids: $ids) {
      count
      errors {
        field
        message
      }
    }
  }
`);
