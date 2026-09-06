/*---------------------------------------------------
-----------------------------------------------------
Filename:	infoline.m
Version:	1.0

Type:		maki
Date:		06. Nov. 2007 - 22:40 
Author:		Martin Poehlmann aka Deimos
E-Mail:		martin@skinconsortium.com
Internet:	www.skinconsortium.com
		www.martin.deimos.de.vu
		
		
		Edited by pjn123
-----------------------------------------------------
---------------------------------------------------*/

#include <lib/std.mi>

Function updateSize();

Global Text label, txt;
Global Layer link;
Global Group sg;
Global Int shift = 0;

System.onScriptLoaded ()
{
	sg = getScriptGroup();

	txt = sg.getObject("text");
	label = sg.getObject("label");
	link = sg.getObject("link");
	
	//txt.setXmlParam("fontsize",sg.getParent().getXMLParam("fontsize"));
	//label.setXmlParam("fontsize",sg.getParent().getXMLParam("fontsize"));

}

txt.onSetVisible(boolean onOff){
	if(onOff){
		link.setXmlparam("tooltip", txt.getText());
		
		updateSize();
		
		if(txt.getText()=="" || txt.getText()=="-"){
			if(getPublicInt("cpro2.tags.50", 1)){
				sg.hide();
				txt.hide();
			}
			else{
				sg.setAlpha(80);
				txt.setText("-");
			}
			
		}
		else sg.setAlpha(253);

	}
}

updateSize(){
	txt.setXmlParam("fontsize", integerToString(getPublicInt("cpro2.tags.textsize", 14)));
	label.setXmlParam("fontsize", integerToString(getPublicInt("cpro2.tags.textsize", 14)));
	sg.setXmlParam("h", integerToString(label.getAutoHeight()));

	int w = label.getAutoWidth() + shift;
	txt.setXmlParam("x", integerToString(w));
	txt.setXmlParam("w", integerToString(-w));
}

txt.onTextChanged (String newtxt){
	link.setXmlparam("tooltip", newtxt);
	
	if(newtxt=="" || newtxt=="-"){
		sg.setAlpha(80);
		txt.setText("-");
	}
	else sg.setAlpha(253);

}

System.onSetXuiParam (String param, String value)
{
	//debug(param+" - "+ value);
	if (strlower(param) == "shift") shift = stringToInteger(value);	
	else if (strlower(param) == "label") label.setText(value);
	else if (strlower(param) == "textsize") updateSize(); //dont use the value
	else if(strleft(strlower(param),7)=="cpro2a_"){ //we use the prefix so that all params gets to the object
		label.setXmlParam(strright(param,strlen(param)-7),value);
	}
	else if(strleft(strlower(param),7)=="cpro2b_"){ //we use the prefix so that all params gets to the object
		txt.setXmlParam(strright(param,strlen(param)-7),value);
	}
	
	//if (strlower(param) == "link") txt.setXmlparam("tooltip", value);
}

label.onTextChanged (String newtxt)
{
	int w = label.getAutoWidth() + shift;
	txt.setXmlParam("x", integerToString(w));
	txt.setXmlParam("w", integerToString(-w));
	sg.setXmlParam("h", integerToString(label.getAutoHeight()));
}