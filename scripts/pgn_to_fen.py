import io
import sys

import chess.pgn

if __name__ == "__main__":
    print("Paste in PGN: (Ctrl-D to confirm)")
    pgn_string = sys.stdin.read()
    pgn = chess.pgn.read_game(io.StringIO(pgn_string))
    uci_moves = [move.uci() for move in pgn.mainline_moves()]
    print("\n" + " ".join(uci_moves))
