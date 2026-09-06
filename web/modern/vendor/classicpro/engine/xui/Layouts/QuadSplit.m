/**
 * QuadSplit
 *
 * @author mpdeimos
 * @date 09/04/16
 */

#include <lib/std.mi>

Function int min (int a, int b);

Function updateLayout();

Global Group scriptGroup;
Global GuiObject coQuad, coModal;
Global int spacing, modalminw, modalminh, modalpreferred;

System.onScriptLoaded ()
{
	scriptGroup = getScriptGroup();
	coQuad = scriptGroup.getObject("quad");
	coModal = scriptGroup.getObject("modal");
	spacing = 0;
}

System.onSetXuiParam (String param, String value)
{
	if (strlower(param) == "quadgroup")
	{
		coQuad.setXmlParam("groupid", value);
		if (value != "")
		{
			coQuad.show();
		}
		else
		{
			coQuad.hide();
		}
	}
	else if (strlower(param) == "modalgroup")
	{
		coModal.setXmlParam("groupid", value);
		if (value != "")
		{
			coModal.show();
		}
		else
		{
			coModal.hide();
		}		
	}
	else if (strlower(param) == "spacing")
	{
		spacing = stringToInteger(value);
		updateLayout();
	}
	else if (strlower(param) == "modalminw")
	{
		modalminw = stringToInteger(value);
		updateLayout();
	}
	else if (strlower(param) == "modalminh")
	{
		modalminh = stringToInteger(value);
		updateLayout();
	}
	else if (strlower(param) == "modalpreferred")
	{
		if (strlower(value) == "right")
		{
			modalpreferred = 2;
		}
		else if (strlower(value) == "bottom")
		{
			modalpreferred = 1;
		}
		else if (strlower(value) == "rightfixed")
		{
			modalpreferred = -2;
		}
		else if (strlower(value) == "bottomfixed")
		{
			modalpreferred = -1;
		}
		else
		{
			modalpreferred = 0;
		}
		
		updateLayout();
	}
}

scriptGroup.onAction (String action, String param, Int x, int y, int p1, int p2, GuiObject source)
{
	if (action == "update")
	{
		updateLayout();
	}
}

scriptGroup.onResize (int x, int y, int w, int h)
{
	updateLayout();
}

updateLayout()
{
	int w = scriptGroup.getWidth();
	int h = scriptGroup.getHeight();

	// initial minimum size of the Quad
	int setQ = min(w,h);

	if (modalpreferred == 1 || modalpreferred == -1) // modal preferred pos == bottom
	{
		if (modalpreferred < 0 || h > w - spacing - modalminw) // set model to bottom
		{

			if (h < setQ + spacing + modalminh)
			{
				setQ = h - spacing - modalminh;
			}
			
			coQuad.setXmlParam("w", integerToString(setQ));
			coQuad.setXmlParam("h", integerToString(setQ));
			coModal.setXmlParam("y", integerToString(setQ+spacing));
			coModal.setXmlParam("x", "0");
			coModal.setXmlParam("h", integerToString(h - (setQ+spacing)));
			coModal.setXmlParam("w", integerToString(w));
		}
		else // set modal to right
		{
			coQuad.setXmlParam("w", integerToString(setQ));
			coQuad.setXmlParam("h", integerToString(setQ));
			coModal.setXmlParam("x", integerToString(setQ+spacing));
			coModal.setXmlParam("y", "0");		
			coModal.setXmlParam("w", integerToString(w - (h+spacing)));
			coModal.setXmlParam("h", integerToString(h));
		}
	}
	else if (modalpreferred == 2 || modalpreferred == -2) // modal preferred pos == right
	{
		if (modalpreferred < 0 || w > h - spacing - modalminh) // set model to right
		{

			if (w < setQ + spacing + modalminw)
			{
				setQ = w - spacing - modalminw;
			}
			
			coQuad.setXmlParam("w", integerToString(setQ));
			coQuad.setXmlParam("h", integerToString(setQ));
			coModal.setXmlParam("x", integerToString(setQ+spacing));
			coModal.setXmlParam("y", "0");		
			coModal.setXmlParam("w", integerToString(w -(setQ+spacing)));
			coModal.setXmlParam("h", integerToString(h));
		}
		else // set modal to bottom
		{
			coQuad.setXmlParam("w", integerToString(setQ));
			coQuad.setXmlParam("h", integerToString(setQ));
			coModal.setXmlParam("y", integerToString(setQ+spacing));
			coModal.setXmlParam("x", "0");
			coModal.setXmlParam("h", integerToString(h - (setQ+spacing)));
			coModal.setXmlParam("w", integerToString(w));
		}
	}
	else // modal preferred pos == smooth
	{
		boolean setToRight = true;
		if (h > w - spacing - modalminw) // set model to bottom
		{
			if (h < setQ + spacing + modalminh)
			{
				setQ = h - spacing - modalminh;
			}

			if (w > setQ + spacing + modalminw) // maybe we want it set to right?
			{
				setQ = w - spacing - modalminw;
				setToRight = true;
			}
			else
			{
				coQuad.setXmlParam("w", integerToString(setQ));
				coQuad.setXmlParam("h", integerToString(setQ));
				coModal.setXmlParam("y", integerToString(setQ+spacing));
				coModal.setXmlParam("x", "0");
				coModal.setXmlParam("h", integerToString(h - (setQ+spacing)));
				coModal.setXmlParam("w", integerToString(w));
				setToRight = false;				
			}
		}
		if (setToRight) // set modal to right
		{
			coQuad.setXmlParam("w", integerToString(setQ));
			coQuad.setXmlParam("h", integerToString(setQ));
			coModal.setXmlParam("x", integerToString(setQ+spacing));
			coModal.setXmlParam("y", "0");		
			coModal.setXmlParam("w", integerToString(w - (setQ+spacing)));
			coModal.setXmlParam("h", integerToString(h));
		}
	}
}

int min (int a, int b)
{
	if (a < b)
	{
		return a;
	}

	return b;
}	