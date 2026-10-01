use super::*;

async fn lamports(solana: &SolanaCookie, address: Pubkey) -> u64 {
    solana
        .context
        .borrow_mut()
        .banks_client
        .get_account(address)
        .await
        .unwrap()
        .unwrap()
        .lamports
}

async fn data_len(solana: &SolanaCookie, address: Pubkey) -> usize {
    solana.get_account_data(address).await.unwrap().len()
}

async fn top_up(solana: &SolanaCookie, payer: TestKeypair, address: Pubkey, amount: u64) {
    solana
        .process_transaction(
            &[solana_sdk::system_instruction::transfer(
                &payer.pubkey(),
                &address,
                amount,
            )],
            Some(&[payer]),
        )
        .await
        .unwrap()
        .result
        .unwrap();
}

#[tokio::test]
async fn test_admin_withdraw_excess_lamports() -> Result<(), TransportError> {
    let context = TestContext::new().await;
    let solana = &context.solana.clone();

    let admin = TestKeypair::new();
    let owner = context.users[0].key;
    let payer = context.users[1].key;
    let mints = &context.mints[0..2];

    let GroupWithTokens { group, tokens, .. } = GroupWithTokensConfig {
        admin,
        payer,
        mints: mints.to_vec(),
        ..GroupWithTokensConfig::default()
    }
    .create(solana)
    .await;

    let mango_v4::accounts::PerpCreateMarket {
        perp_market,
        bids,
        asks,
        event_queue,
        ..
    } = send_tx(
        solana,
        PerpCreateMarketInstruction {
            group,
            admin,
            payer,
            perp_market_index: 0,
            quote_lot_size: 10,
            base_lot_size: 100,
            ..PerpCreateMarketInstruction::with_new_book_and_queue(&solana, &tokens[1]).await
        },
    )
    .await
    .unwrap();

    let mango_v4::accounts::PerpCreateMarket {
        perp_market: other_perp_market,
        ..
    } = send_tx(
        solana,
        PerpCreateMarketInstruction {
            group,
            admin,
            payer,
            perp_market_index: 1,
            quote_lot_size: 10,
            base_lot_size: 100,
            ..PerpCreateMarketInstruction::with_new_book_and_queue(&solana, &tokens[1]).await
        },
    )
    .await
    .unwrap();

    let fermi_account = create_funded_account(
        &solana,
        group,
        owner,
        0,
        &context.users[1],
        &mints[0..1],
        10,
        0,
    )
    .await;

    let destination = TestKeypair::new().pubkey();
    top_up(solana, payer, destination, 1_000_000).await;

    let excess = 123_456_789;
    let targets = [
        (group, None),
        (tokens[0].bank, None),
        (tokens[0].mint_info, None),
        (perp_market, None),
        (bids, Some(perp_market)),
        (asks, Some(perp_market)),
        (event_queue, Some(perp_market)),
    ];

    //
    // TEST: each supported protocol account returns exactly its excess
    //
    for (target, linked_perp_market) in targets {
        top_up(solana, payer, target, excess).await;
        let min_balance = solana.rent.minimum_balance(data_len(solana, target).await);
        let target_before = lamports(solana, target).await;
        assert_eq!(target_before, min_balance + excess);
        let destination_before = lamports(solana, destination).await;

        send_tx(
            solana,
            AdminWithdrawExcessLamportsInstruction {
                group,
                admin,
                target,
                destination,
                perp_market: linked_perp_market,
            },
        )
        .await
        .unwrap();

        assert_eq!(lamports(solana, target).await, min_balance);
        assert_eq!(
            lamports(solana, destination).await,
            destination_before + excess
        );
    }

    //
    // TEST: no excess is a no-op
    //
    let destination_before = lamports(solana, destination).await;
    send_tx(
        solana,
        AdminWithdrawExcessLamportsInstruction {
            group,
            admin,
            target: perp_market,
            destination,
            perp_market: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(lamports(solana, destination).await, destination_before);

    top_up(solana, payer, bids, excess).await;
    top_up(solana, payer, fermi_account, excess).await;

    //
    // TEST: user FermiAccounts are rejected
    //
    send_tx_expect_error!(
        solana,
        AdminWithdrawExcessLamportsInstruction {
            group,
            admin,
            target: fermi_account,
            destination,
            perp_market: None,
        },
        FermiError::AdminWithdrawExcessLamportsUnsupportedAccount,
    );

    //
    // TEST: a book side needs the PerpMarket that references it
    //
    send_tx_expect_error!(
        solana,
        AdminWithdrawExcessLamportsInstruction {
            group,
            admin,
            target: bids,
            destination,
            perp_market: None,
        },
        FermiError::AdminWithdrawExcessLamportsUnsupportedAccount,
    );
    send_tx_expect_error!(
        solana,
        AdminWithdrawExcessLamportsInstruction {
            group,
            admin,
            target: bids,
            destination,
            perp_market: Some(other_perp_market),
        },
        FermiError::AdminWithdrawExcessLamportsUnsupportedAccount,
    );

    //
    // TEST: only the group admin may withdraw
    //
    assert!(send_tx(
        solana,
        AdminWithdrawExcessLamportsInstruction {
            group,
            admin: payer,
            target: bids,
            destination,
            perp_market: Some(perp_market),
        },
    )
    .await
    .is_err());

    //
    // TEST: destination cannot be the target
    //
    send_tx_expect_error!(
        solana,
        AdminWithdrawExcessLamportsInstruction {
            group,
            admin,
            target: tokens[0].bank,
            destination: tokens[0].bank,
            perp_market: None,
        },
        FermiError::SomeError,
    );

    //
    // TEST: the ix gate disables it
    //
    send_tx(
        solana,
        IxGateSetInstruction {
            group,
            admin,
            ix_gate: 1u128 << IxGate::AdminWithdrawExcessLamports as u128,
        },
    )
    .await
    .unwrap();
    send_tx_expect_error!(
        solana,
        AdminWithdrawExcessLamportsInstruction {
            group,
            admin,
            target: bids,
            destination,
            perp_market: Some(perp_market),
        },
        FermiError::IxIsDisabled,
    );

    Ok(())
}
