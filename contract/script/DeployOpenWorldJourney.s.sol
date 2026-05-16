// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Script, console} from "forge-std/Script.sol";
import {OpenWorldJourney, IntelligentData} from "../src/OpenWorldJourney.sol";

/**
 * @notice Deploy OpenWorldJourney to 0G Mainnet (chain ID 16661).
 *
 * Usage:
 *   forge script script/DeployOpenWorldJourney.s.sol \
 *     --rpc-url https://evmrpc.0g.ai \
 *     --private-key $OPERATOR_PRIVATE_KEY \
 *     --broadcast \
 *     --verify
 *
 * After deploying, set JOURNEY_CONTRACT_ADDRESS in backend/.env.
 */
contract DeployOpenWorldJourney is Script {
  function run() external {
    vm.startBroadcast();

    OpenWorldJourney journey = new OpenWorldJourney();
    console.log("OpenWorldJourney deployed at:", address(journey));
    console.log("Owner (operator):", msg.sender);
    console.log("Chain ID:", block.chainid);

    // Mint genesis token to deployer as a smoke test
    // In production, tokens are minted by the Rust agent via mintAndRecord()
    uint256 tokenId = journey.mintJourney(
      msg.sender,
      msg.sender,
      "genesis-session-000",
      "OpenWorld Genesis Journey - contract deployment smoke test"
    );
    console.log("Genesis token minted - tokenId:", tokenId);

    (bool hasMemory, bool hasReport, bool complete) = journey.verifyJourney(tokenId);
    console.log("Genesis token - hasMemory:", hasMemory);
    console.log("Genesis token - hasReport:", hasReport);
    console.log("Genesis token - complete:", complete);

    vm.stopBroadcast();

    console.log("");
    console.log("Next steps:");
    console.log("  1. Add to backend/.env:");
    console.log("     JOURNEY_CONTRACT_ADDRESS=", address(journey));
    console.log("  2. The Rust agent will call mintAndRecord() after each completed trip.");
  }
}
